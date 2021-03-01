mod error;
mod data_type;
mod db;

use actix_web::{App, HttpResponse, HttpServer, middleware, web, HttpRequest};

use std::path::{PathBuf};
use actix_files::NamedFile;
use error::MyError;
use data_type::*;
use db::Pool;

async fn get_album_artwork(web::Path((id,)): web::Path<(i64, )>, pool: web::Data<Pool>) -> Result<NamedFile, MyError> {
    let c = pool.get()?;
    let a = db::find_album_artwork(&c, id)?;
    let file = actix_files::NamedFile::open(a.path)?;
    Ok(file)
}

async fn get_album_tracks(web::Path((id,)): web::Path<(AlbumId, )>, pool: web::Data<Pool>) -> Result<HttpResponse, MyError> {
    let c = pool.get()?;
    let tracks = db::find_album_tracks(&c, id)?;
    Ok(HttpResponse::Ok().json(Response { values: tracks }))
}

async fn get_album_track_audio(req: HttpRequest, web::Path((album_id,track_id)): web::Path<(AlbumId, TrackId, )>, pool: web::Data<Pool>) -> Result<HttpResponse, actix_web::Error> {
    let c = pool.get().map_err(MyError::from)?;
    let tracks = db::find_album_tracks(&c, album_id)?;

    if let Some(track) = tracks.iter().find(|t| t.id == track_id) {
        let file = actix_files::NamedFile::open(&track.path)?;
        actix_files::NamedFile::open(file.path())?.into_response(&req)
    } else {
        Ok(HttpResponse::NotFound().body(format!("Track {:?} for album {:?} not found", track_id, album_id)))
    }
}

async fn get_artist_full_albums(pool: web::Data<Pool>, web::Path((artist_id,)): web::Path<(ArtistId, )>) -> Result<HttpResponse, MyError> {
    let conn = pool.get()?;
    let resp = db::find_artist_full_albums(&conn, artist_id)?;
    Ok(HttpResponse::Ok().json(Response { values: resp }))
}


async fn list_artists(pool: web::Data<Pool>) -> Result<HttpResponse, MyError> {
    let conn = pool.get()?;
    let res = db::find_artists(&conn)?;
    Ok(HttpResponse::Ok().json(Response { values: res }))
}



#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let manager = r2d2_sqlite::SqliteConnectionManager::file("library.db");
    let pool = r2d2::Pool::new(manager).expect("could not open db");

    let base_dir = PathBuf::from("/home/simao/MusicBeets");

    let mut c = pool.get().expect("Could not get pool connection");

    db::clean_db(&c).expect("Could not clean db");
    db::reload_db(&base_dir, &mut c).expect("Could not reload db");

    HttpServer::new(move || { App::new()
        .wrap(middleware::Logger::default())
        .data(pool.clone())
        .data(base_dir.clone())
        .route("/albums/{id}/artwork", web::get().to(get_album_artwork))
        .route("/albums/{id}/tracks", web::get().to(get_album_tracks))
        .route("/albums/{id}/tracks/{track_id}/audio", web::get().to(get_album_track_audio))
        .route("/artists", web::get().to(list_artists))
        .route("/artists/{id}/full-albums", web::get().to(get_artist_full_albums))

    }).bind("0.0.0.0:3030")
        .unwrap()
        .run()
        .await
}
