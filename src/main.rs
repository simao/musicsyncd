use actix_web::{App, http, HttpResponse, HttpServer, middleware, web, HttpRequest};

use rusqlite::{params, ToSql};
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Serialize, Deserialize};
use std::path::{PathBuf};
use failure::{Error, AsFail};
use actix_files::NamedFile;
use rusqlite::types::{FromSql, FromSqlResult, ValueRef, FromSqlError};
use std::fmt::Display;
use failure::_core::fmt::Formatter;
use rusqlite::NO_PARAMS;

pub type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;
pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

use rusqlite::types::Type::Null;

use audiotags::Tag;


#[derive(Serialize, Deserialize, Debug)]
struct Artist {
    id: i64,
    name: String
}

struct AlbumArtwork {
    album_id: i64,
    path: std::path::PathBuf
}

#[derive(Serialize, Deserialize, Debug)]
struct Album {
    id: i64,
    title: String,
    artist: Artist,
}

#[derive(Serialize, Debug)]
struct FullAlbum {
    album: Album,
    tracks: Vec<Track>
}

#[derive(Serialize, Debug)]
struct Response<T : Serialize> {
    values: Vec<T>
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct TrackId(i64);

impl Display for TrackId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Serialize, Debug)]
struct Track {
    id: TrackId,
    title: Option<String>,
    filename: String,
    #[serde(skip)]
    path: PathBuf
}

struct PathBlob(PathBuf);

impl FromSql for PathBlob {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let path_str = value.as_str()?;
        Ok(PathBlob(PathBuf::from(path_str)))
    }
}

use rusqlite::types::{ToSqlOutput};
use std::ffi::OsStr;
// use rusqlite::{OptionalExtension};


impl ToSql for PathBlob {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self.0.to_str() {
            None => Ok(ToSqlOutput::from(rusqlite::types::Null)),
            Some(s) => Ok(ToSqlOutput::from(s.to_owned()))
        }
    }
}

fn find_artist_full_albums(c: &Connection, artist_id: i64) -> Result<Vec<FullAlbum>, Error>{
    let mut artist_albums = find_albums(&c, Some(artist_id))?;

    let mut res: Vec<FullAlbum> = vec![];

    for album in artist_albums.drain(0..) {
        let tracks = find_album_tracks(&c, album.id)?;

        let fa = FullAlbum { album, tracks };
        res.push(fa)
    }

    Ok(res)
}

fn find_album_tracks(c: &Connection, album_id: i64) -> Result<Vec<Track>, Error> {
    let mut stmt = c.prepare(
        "select id, title, full_path from tracks where album_id = ?")?;

    let rows = stmt.query_map(params![album_id], |row| {
        let path: PathBlob = row.get("full_path")?;
        let filename = path.0.file_name().and_then(|s| s.to_str()).expect("Invalid track path").to_owned(); //TODO

        Ok(
            Track {
                id: TrackId(row.get("id")?),
                title: row.get("title")?,
                path: path.0,
                filename
            }
        )
    })?;

    let mut res = vec![];

    for row in rows {
        res.push(row?);
    }

    return Ok(res)
}

fn find_album_artwork(c: &Connection, album_id: i64) -> Result<AlbumArtwork, Error> {
    let mut stmt = c.prepare("select art_path FROM albums where id = ?")?;

    let row = stmt.query_row(params![album_id], |row| {
        let path: PathBlob = row.get("art_path")?;

        Ok(
            AlbumArtwork {
                album_id,
                path: path.0
            }
        )
    })?;

    Ok(row)
}

fn find_albums(c: &Connection, artist_id: Option<i64>) -> Result<Vec<Album>, Error> {
    let mut sql = "\
      SELECT at.id artist_id, at.name, al.id album_id, al.title \
      from artists at INNER JOIN albums al \
      on al.artist_id = at.id".to_owned();

    let aid: i64;
    let mut params: Vec<&dyn ToSql> = vec![];

    if let Some(_aid) = artist_id {
        aid = _aid;
        sql = sql + " WHERE artist_id = ?";
        params.push(&aid);
    }

    let mut stmt = c.prepare(&sql)?;

    let rows = stmt.query_map(params, |row: &rusqlite::Row| {
        let artist_id: i64 = row.get("artist_id")?;
        let name: String = row.get("name")?;
        let album_id: i64 = row.get("album_id")?;
        let title: String = row.get("title")?;

        let artist = Artist { id: artist_id, name };

        Ok(Album { id: album_id, artist, title })
    })?;

    let mut res = vec![];

    for album in rows {
        if let Err(err) = album {
            log::warn!("Could not read artist from db: {}", err)
        } else {
            res.push(album?);
        }
    }

    Ok(res)
}

fn find_artists(c: &Connection) -> Result<Vec<Artist>, Error> {
    let mut res: Vec<Artist> = vec![];

    let mut stmt = c.prepare("SELECT id, name from artists")?;

    let rows = stmt.query_map(NO_PARAMS, |row: &rusqlite::Row| {
        let id = row.get(0)?;
        let name = row.get(1)?;
        Ok(Artist { id, name })
    })?;

    for artist in rows {
        if let Err(err) = artist {
            log::warn!("Could not read artist from db: {}", err)
        } else {
            res.push(artist?);
        }
    }

    Ok(res)
}

async fn get_album_artwork(path: web::Path<(i64, )>, pool: web::Data<Pool>) -> Result<NamedFile, Error> {
    let c = pool.get()?;
    let a = find_album_artwork(&c, path.0)?;
    let file = actix_files::NamedFile::open(a.path)?;
    Ok(file)
}

async fn get_album_tracks(path: web::Path<(i64, )>, pool: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let c = pool.get()?;
    let tracks = find_album_tracks(&c, path.0)?;
    Ok(HttpResponse::Ok().json(Response { values: tracks }))
}

async fn get_album_track_audio(req: HttpRequest, path: web::Path<(i64, TrackId, )>, pool: web::Data<Pool>) -> Result<HttpResponse, actix_web::Error> {
    let c = pool.get().map_err(failure::Error::from)?;
    let tracks = find_album_tracks(&c, path.0)?;

    if let Some(track) = tracks.iter().find(|t| t.id == path.1) {
        let file = actix_files::NamedFile::open(&track.path)?;
        actix_files::NamedFile::open(file.path())?.into_response(&req)
    } else {
        Ok(HttpResponse::NotFound().body(format!("Track {} for album {} not found", path.1, path.0)))
    }
}

async fn get_artist_full_albums(pool: web::Data<Pool>, path: web::Path<(i64, )>) -> Result<HttpResponse, Error> {
    let conn = pool.get()?;
    let resp = find_artist_full_albums(&conn, path.0)?;
    Ok(HttpResponse::Ok().json(Response { values: resp }))
}


async fn list_artists(pool: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let conn = pool.get()?;
    let res = find_artists(&conn)?;
    Ok(HttpResponse::Ok().json(Response { values: res }))
}

#[derive(Clone, Debug)]
struct BaseDir(PathBuf);


fn clean_db(c: &Connection) -> Result<(), Error> {
    c.execute_batch("\
      DROP TABLE IF EXISTS artists;\
      DROP TABLE IF EXISTS albums;\
      DROP TABLE IF EXISTS tracks;\
      create table artists (id integer primary key autoincrement, name text not null);\
      create table albums (id integer primary key autoincrement, title text not null, artist_id integer not null, art_path TEXT NULL);\
      create table tracks (id integer primary key autoincrement, title text null, album_id integer not null, full_path TEXT NOT NULL);\
    ")?;

    log::info!("Clean db finished");

    Ok(())
}

// TODO: Delete old entries, keep ids
fn reload_db(base_dir: &BaseDir, c: &mut Connection) -> Result<(), Error> {
    log::info!("Reloading db using {}", base_dir.0.display());

    let paths = std::fs::read_dir(&base_dir.0)?;

    let mut artist_paths: Vec<(i64, PathBuf)> = vec![];

    let tx = c.transaction()?;

    for path in paths {
        let artist_path = path?.path();

        if ! artist_path.is_dir() {
            log::debug!("Skipping {:?}", artist_path);
            continue;
        }

        let name = format!("{}", artist_path.file_name().unwrap().to_string_lossy()); // TODO: Unwrap
        tx.execute("INSERT INTO artists (name) VALUES (?)", params![name])?;

        let last_id = tx.last_insert_rowid();

        artist_paths.push((last_id, artist_path));
    }

    tx.commit()?;

    let tx = c.transaction()?;

    for (aid, artist_path) in artist_paths {
        let mut dir = base_dir.0.clone();
        dir.push(artist_path);

        log::debug!("Opening {:?}", dir);

        if !dir.is_dir() {
            continue;
        }

        let album_dirs = std::fs::read_dir(dir)?;

        for adir in album_dirs {
            let apath = adir?.path();
            let name = apath.file_name().unwrap().to_string_lossy();

            let artwork_path = apath.join("cover.jpg"); // TODO: Find more extensions

            let db_artwork_path = if artwork_path.exists() {
                Some(PathBlob(artwork_path))
            } else {
                None
            };

            tx.execute("INSERT INTO albums (title, artist_id, art_path) VALUES (?1, ?2, ?3)", params![name, aid, db_artwork_path])?;

            let album_id = tx.last_insert_rowid();

            if ! apath.is_dir() {
                continue;
            }

            log::debug!("Searching for tracks in {:?}", apath);

            let track_paths = std::fs::read_dir(apath)?;

            for tentry in track_paths {
                let tpath = tentry?.path();

                if ! tpath.is_file() || tpath.extension().unwrap() == "jpg" {
                    continue;
                }

                log::debug!("Reading tags for {:?}", tpath);

                let t = Tag::default().read_from_path(&tpath);

                match t {
                    Ok(audio_tags) =>
                        tx.execute("INSERT INTO tracks (title, album_id, full_path) VALUES (?1, ?2, ?3)",
                                   params![audio_tags.title(), album_id, PathBlob(tpath)])?,
                    Err(err) => {
                        log::debug!("Could not read metadata from {:?}: {:?}", tpath, err);
                        0 as usize
                    }

                };
            }
        }
    }


    tx.commit()?;


    log::info!("Finished reloading db");

    Ok(())
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let manager = SqliteConnectionManager::file("library.db");
    let pool = r2d2::Pool::new(manager).expect("could not open db");

    let base_dir = BaseDir(PathBuf::from("/home/simao/MusicBeets"));

    let mut c = pool.get().expect("Could not get pool connection");

    clean_db(&c).expect("Could not clean db");
    reload_db(&base_dir, &mut c).expect("Could not reload db");

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
