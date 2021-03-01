mod error;
mod data_type;
mod db;

use actix_web::{App, HttpResponse, HttpServer, middleware, web, HttpRequest};

use std::path::PathBuf;
use actix_files::NamedFile;
use error::MSError;
use data_type::*;
use db::Pool;

async fn get_album_artwork(web::Path((id,)): web::Path<(i64, )>, pool: web::Data<Pool>) -> Result<NamedFile, MSError> {
    let c = pool.get()?;
    let a = db::find_album_artwork(&c, id)?;
    let file = actix_files::NamedFile::open(a.path)?;
    Ok(file)
}

async fn get_album_tracks(web::Path((id,)): web::Path<(AlbumId, )>, pool: web::Data<Pool>) -> Result<HttpResponse, MSError> {
    let c = pool.get()?;
    let tracks = db::find_album_tracks(&c, id)?;
    Ok(HttpResponse::Ok().json(Response { values: tracks }))
}

async fn get_album_track_audio(req: HttpRequest, web::Path((album_id,track_id)): web::Path<(AlbumId, TrackId, )>, pool: web::Data<Pool>) -> Result<HttpResponse, actix_web::Error> {
    let c = pool.get().map_err(MSError::from)?;
    let tracks = db::find_album_tracks(&c, album_id)?;

    if let Some(track) = tracks.iter().find(|t| t.id == track_id) {
        let file = actix_files::NamedFile::open(&track.path)?;
        actix_files::NamedFile::open(file.path())?.into_response(&req)
    } else {
        Ok(HttpResponse::NotFound().body(format!("Track {:?} for album {:?} not found", track_id, album_id)))
    }
}

async fn get_artist_full_albums(pool: web::Data<Pool>, web::Path((artist_id,)): web::Path<(ArtistId, )>) -> Result<HttpResponse, MSError> {
    let conn = pool.get()?;
    let resp = db::find_artist_full_albums(&conn, artist_id)?;
    Ok(HttpResponse::Ok().json(Response { values: resp }))
}


async fn list_artists(pool: web::Data<Pool>) -> Result<HttpResponse, MSError> {
    let conn = pool.get()?;
    let res = db::find_artists(&conn)?;
    Ok(HttpResponse::Ok().json(Response { values: res }))
}


use structopt::StructOpt;
use anyhow::Context;

#[derive(StructOpt, Debug)]
#[structopt(name = "musicsyncd")]
struct Opt {
    /// Directory where to save music database. Defaults to using XDG_DIR for cache
    #[structopt(short, long, parse(from_os_str))]
    cache_dir: Option<PathBuf>,

    /// Music Directory where files are organize by Artist/Album/Tracks
    #[structopt(short, long, parse(from_os_str))]
    music_dir: PathBuf,

    /// scan music directory and find artists albums and tracks
    #[structopt(long = "no-reload", parse(from_flag = std::ops::Not::not))]
    reload: bool
}

fn load_user_database(opt: &Opt) -> Result<r2d2_sqlite::SqliteConnectionManager, MSError> {
    let base_dirs = directories::BaseDirs::new().context("Could not initialize user base dirs")?;

    let cache_dir: PathBuf = if let Some(ref d) = opt.cache_dir {
        d.clone()
    } else {
        base_dirs.cache_dir().join("musicsyncd").into()
    };

    if ! cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir).context("Could not create directories")?;
    }

    let cache_file = cache_dir.join("musicsyncd.db");

    log::info!("Using {:?} as cache db", cache_file);

    Ok(r2d2_sqlite::SqliteConnectionManager::file(cache_file))
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    log::debug!("Using options: {:#?}", opt);

    let manager = load_user_database(&opt).expect("Could not load user database");
    let pool = r2d2::Pool::new(manager).expect("could not open db");
    let mut c = pool.get().expect("Could not get pool connection");

    if opt.reload {
        db::clean_db(&c).expect("Could not clean db");
        db::reload_db(&opt.music_dir, &mut c).expect("Could not reload db");
    }

    HttpServer::new(move || { App::new()
        .wrap(middleware::Logger::default())
        .data(pool.clone())
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
