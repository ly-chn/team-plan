mod db;
mod holiday;

use actix_files as fs;
use actix_session::{Session, SessionMiddleware};
use actix_session::storage::CookieSessionStore;
use actix_web::{web, App, HttpResponse, HttpServer, middleware};
use actix_web::cookie::Key;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use log::{info, warn};

struct AppState {
    db: Arc<Mutex<db::Database>>,
}

// ─── Holiday auto-refresh (startup + daily at 01:00) ───

async fn refresh_holidays(db: &Mutex<db::Database>) {
    info!("Refreshing holidays from API...");
    let entries = holiday::fetch_current_and_next().await;
    if entries.is_empty() {
        warn!("No holiday entries fetched");
        return;
    }
    let db = db.lock().unwrap();
    match db.import_holidays(&entries) {
        Ok(count) => info!("Imported {} holiday entries", count),
        Err(e) => warn!("Failed to import holidays: {}", e),
    }
}

fn spawn_holiday_scheduler(db: Arc<Mutex<db::Database>>) {
    tokio::spawn(async move {
        // Initial fetch on startup
        refresh_holidays(&db).await;

        loop {
            // Calculate duration until next 01:00
            let now = chrono::Local::now();
            let next_1am = now.date_naive().and_hms_opt(1, 0, 0).unwrap();
            let next_1am = if now.time() >= chrono::NaiveTime::from_hms_opt(1, 0, 0).unwrap() {
                next_1am + chrono::Duration::days(1)
            } else {
                next_1am
            };
            let wait = (next_1am - now.naive_local()).to_std().unwrap_or(std::time::Duration::from_secs(3600));
            info!("Next holiday refresh in {:?}", wait);
            tokio::time::sleep(wait).await;
            refresh_holidays(&db).await;
        }
    });
}

#[derive(Deserialize)]
struct LoginReq {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct RegisterReq {
    username: String,
    name: String,
    password: String,
}

#[derive(Deserialize)]
struct TaskReq {
    user_id: i64,
    iteration_name: String,
    start_date: String,
    end_date: String,
    hours_review: f64,
    hours_coding: f64,
    hours_testing: f64,
    hours_deploy: f64,
    hours_tracking: f64,
    hours_other: f64,
}

#[derive(Deserialize)]
struct LeaveReq {
    user_id: i64,
    start_date: String,
    hours: f64,
}

#[derive(Deserialize)]
struct OvertimeReq {
    user_id: i64,
    start_date: String,
    hours: f64,
}

#[derive(Deserialize)]
struct VisibleReq {
    start: String,
    end: String,
}

fn get_user_id(session: &Session) -> Option<i64> {
    session.get::<i64>("user_id").ok().flatten()
}

// ─── Auth ───

async fn register(data: web::Data<AppState>, body: web::Json<RegisterReq>) -> HttpResponse {
    let db = data.db.lock().unwrap();
    let hash = match bcrypt::hash(&body.password, 4) {
        Ok(h) => h,
        Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    };
    match db.create_user(&body.username, &body.name, &hash) {
        Ok(id) => HttpResponse::Ok().json(serde_json::json!({"id": id})),
        Err(e) => {
            if e.to_string().contains("UNIQUE") {
                HttpResponse::Conflict().json(serde_json::json!({"error": "用户名已存在"}))
            } else {
                HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()}))
            }
        }
    }
}

async fn login(data: web::Data<AppState>, body: web::Json<LoginReq>, session: Session) -> HttpResponse {
    let db = data.db.lock().unwrap();
    match db.get_user_by_username(&body.username) {
        Ok(Some(user)) => {
            if bcrypt::verify(&body.password, &user.password_hash).unwrap_or(false) {
                session.insert("user_id", user.id).unwrap();
                session.insert("user_name", &user.name).unwrap();
                HttpResponse::Ok().json(serde_json::json!({"id": user.id, "name": user.name}))
            } else {
                HttpResponse::Unauthorized().json(serde_json::json!({"error": "密码错误"}))
            }
        }
        Ok(None) => HttpResponse::Unauthorized().json(serde_json::json!({"error": "用户不存在"})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn logout(session: Session) -> HttpResponse {
    session.purge();
    HttpResponse::Ok().json(serde_json::json!({"ok": true}))
}

async fn me(session: Session) -> HttpResponse {
    if let Some(uid) = get_user_id(&session) {
        let name: String = session.get::<String>("user_name").ok().flatten().unwrap_or_default();
        HttpResponse::Ok().json(serde_json::json!({"id": uid, "name": name}))
    } else {
        HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}))
    }
}

// ─── Users ───

async fn list_users(data: web::Data<AppState>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.list_users() {
        Ok(users) => {
            let safe: Vec<_> = users.iter().map(|u| serde_json::json!({"id": u.id, "username": u.username, "name": u.name})).collect();
            HttpResponse::Ok().json(safe)
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// ─── Holidays ───

async fn list_holidays(data: web::Data<AppState>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.list_holidays() {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// ─── Tasks ───

async fn list_tasks(data: web::Data<AppState>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.list_tasks() {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn list_tasks_visible(data: web::Data<AppState>, query: web::Query<VisibleReq>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.list_tasks_visible(&query.start, &query.end) {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn create_task(data: web::Data<AppState>, body: web::Json<TaskReq>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.create_task(
        body.user_id, &body.iteration_name, &body.start_date, &body.end_date,
        body.hours_review, body.hours_coding, body.hours_testing,
        body.hours_deploy, body.hours_tracking, body.hours_other,
    ) {
        Ok(id) => HttpResponse::Ok().json(serde_json::json!({"id": id})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn update_task(data: web::Data<AppState>, path: web::Path<i64>, body: web::Json<TaskReq>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.update_task(
        path.into_inner(), body.user_id, &body.iteration_name, &body.start_date, &body.end_date,
        body.hours_review, body.hours_coding, body.hours_testing,
        body.hours_deploy, body.hours_tracking, body.hours_other,
    ) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn delete_task(data: web::Data<AppState>, path: web::Path<i64>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.delete_task(path.into_inner()) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn recent_iterations(data: web::Data<AppState>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.recent_iteration_names() {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// ─── Leave ───

async fn list_leave(data: web::Data<AppState>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.list_leave() {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn create_leave(data: web::Data<AppState>, body: web::Json<LeaveReq>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.create_leave(body.user_id, &body.start_date, body.hours) {
        Ok(id) => HttpResponse::Ok().json(serde_json::json!({"id": id})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn update_leave(data: web::Data<AppState>, path: web::Path<i64>, body: web::Json<LeaveReq>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.update_leave(path.into_inner(), body.user_id, &body.start_date, body.hours) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn delete_leave(data: web::Data<AppState>, path: web::Path<i64>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.delete_leave(path.into_inner()) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// ─── Overtime ───

async fn list_overtime(data: web::Data<AppState>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.list_overtime() {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn create_overtime(data: web::Data<AppState>, body: web::Json<OvertimeReq>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.create_overtime(body.user_id, &body.start_date, body.hours) {
        Ok(id) => HttpResponse::Ok().json(serde_json::json!({"id": id})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn update_overtime(data: web::Data<AppState>, path: web::Path<i64>, body: web::Json<OvertimeReq>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.update_overtime(path.into_inner(), body.user_id, &body.start_date, body.hours) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn delete_overtime(data: web::Data<AppState>, path: web::Path<i64>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.delete_overtime(path.into_inner()) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// ─── Report ───

async fn get_report(data: web::Data<AppState>, session: Session) -> HttpResponse {
    if get_user_id(&session).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "未登录"}));
    }
    let db = data.db.lock().unwrap();
    match db.get_report() {
        Ok(r) => HttpResponse::Ok().json(r),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let db = Arc::new(Mutex::new(db::Database::open("mimo.db").expect("Failed to open database")));

    // Start holiday auto-refresh scheduler
    spawn_holiday_scheduler(db.clone());

    let data = web::Data::new(AppState {
        db: db.clone(),
    });

    let secret_key = Key::from(&[0u8; 64]);

    info!("Starting server at http://127.0.0.1:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(middleware::Logger::default())
            .wrap(SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                .cookie_secure(false)
                .build())
            .service(
                web::scope("/api")
                    .route("/register", web::post().to(register))
                    .route("/login", web::post().to(login))
                    .route("/logout", web::post().to(logout))
                    .route("/me", web::get().to(me))
                    .route("/users", web::get().to(list_users))
                    .route("/holidays", web::get().to(list_holidays))
                    .route("/tasks", web::get().to(list_tasks))
                    .route("/tasks/visible", web::get().to(list_tasks_visible))
                    .route("/tasks", web::post().to(create_task))
                    .route("/tasks/{id}", web::put().to(update_task))
                    .route("/tasks/{id}", web::delete().to(delete_task))
                    .route("/iterations/recent", web::get().to(recent_iterations))
                    .route("/leave", web::get().to(list_leave))
                    .route("/leave", web::post().to(create_leave))
                    .route("/leave/{id}", web::put().to(update_leave))
                    .route("/leave/{id}", web::delete().to(delete_leave))
                    .route("/overtime", web::get().to(list_overtime))
                    .route("/overtime", web::post().to(create_overtime))
                    .route("/overtime/{id}", web::put().to(update_overtime))
                    .route("/overtime/{id}", web::delete().to(delete_overtime))
                    .route("/report", web::get().to(get_report))
            )
            .service(fs::Files::new("/", "static").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
