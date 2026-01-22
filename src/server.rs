use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use actix_multipart::Multipart;
use futures::{StreamExt, TryStreamExt};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;
use std::sync::Arc;
use actix_cors::Cors;

mod job_manager;
use job_manager::{JobManager, JobStatus};

struct AppState {
    job_manager: Arc<JobManager>,
}

#[get("/api/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json("Server is running")
}

#[post("/api/upload")]
async fn upload_file(mut payload: Multipart, data: web::Data<AppState>) -> impl Responder {
    let mut file_path = PathBuf::from("uploads");
    std::fs::create_dir_all(&file_path).unwrap_or_default();
    
    let job_id = Uuid::new_v4().to_string();
    
    // Try to detect extension from original filename
    let mut extension = "csv".to_string(); // Default

    // We need to parse multipart to get filename first? actix-multipart iterates fields.
    // The loop handles fields. We can't know filename before iterating.
    // But we write to file inside the loop.
    // Let's create a temporary path or update logic inside loop.
    
    // SIMPLER LOGIC: Write to a temp file, then rename once we know the filename? 
    // OR just handle the file field specifically.
    
    // Let's defer file creation until we find the field.
    let mut saved_filename = String::new();


    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();
        if content_disposition.get_name().unwrap_or("") == "file" {
            // Get extension
            if let Some(original_name) = content_disposition.get_filename() {
                if original_name.ends_with(".xlsx") || original_name.ends_with(".XLSX") {
                    extension = "xlsx".to_string();
                } else if original_name.ends_with(".xls") {
                    extension = "xls".to_string();
                }
            }
            
            let filename = format!("{}.{}", job_id, extension);
            file_path.push(&filename);
            saved_filename = filename.clone();

            let mut f = std::fs::File::create(&file_path).unwrap();
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                f.write_all(&data).unwrap();
            }
        }
    }

    // Determine output path
    let mut output_path = PathBuf::from("outputs");
    std::fs::create_dir_all(&output_path).unwrap_or_default();
    output_path.push(format!("results_{}.csv", job_id));

    // Start Job
    data.job_manager.start_job(job_id.clone(), file_path.clone(), output_path.clone());

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "job_id": job_id,
        "message": "File uploaded and job queued."
    }))
}

#[get("/api/status/{job_id}")]
async fn get_status(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let job_id = path.into_inner();
    let jobs = data.job_manager.jobs.lock().unwrap();

    if let Some(job) = jobs.get(&job_id) {
        HttpResponse::Ok().json(job)
    } else {
        HttpResponse::NotFound().json("Job not found")
    }
}

#[get("/api/download/{job_id}")]
async fn download_result(path: web::Path<String>) -> impl Responder {
    let job_id = path.into_inner();
    let mut output_path = PathBuf::from("outputs");
    output_path.push(format!("results_{}.csv", job_id));

    if output_path.exists() {
        let content = std::fs::read_to_string(output_path).unwrap();
        HttpResponse::Ok()
            .content_type("text/csv")
            .append_header(("Content-Disposition", format!("attachment; filename=\"results_{}.csv\"", job_id)))
            .body(content)
    } else {
        HttpResponse::NotFound().body("Result file not generated yet.")
    }
}


#[post("/api/pause/{job_id}")]
async fn pause_job(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let job_id = path.into_inner();
    if data.job_manager.send_control(&job_id, "pause") {
        HttpResponse::Ok().json("Job paused")
    } else {
        HttpResponse::NotFound().json("Job not found")
    }
}

#[post("/api/resume/{job_id}")]
async fn resume_job(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let job_id = path.into_inner();
    if data.job_manager.send_control(&job_id, "resume") {
        HttpResponse::Ok().json("Job resumed")
    } else {
        HttpResponse::NotFound().json("Job not found")
    }
}

#[post("/api/stop/{job_id}")]
async fn stop_job(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let job_id = path.into_inner();
    if data.job_manager.send_control(&job_id, "stop") {
        HttpResponse::Ok().json("Job stopped")
    } else {
        HttpResponse::NotFound().json("Job not found")
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let job_manager = Arc::new(JobManager::new());
    let state = web::Data::new(AppState { job_manager });

    log::info!("Starting Web Server at http://0.0.0.0:8080");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .app_data(state.clone())
            .service(health_check)
            .service(upload_file)
            .service(get_status)
            .service(download_result)
            .service(pause_job)
            .service(resume_job)
            .service(stop_job)
            .service(actix_files::Files::new("/", "./frontend/dist").index_file("index.html"))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
