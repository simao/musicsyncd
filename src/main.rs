use actix_web::{App, http, HttpResponse, HttpServer, middleware, web, HttpRequest};

use rusqlite::{params};
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Serialize, Deserialize};
use std::path::{PathBuf};
use failure::{Error, AsFail};
use actix_files::NamedFile;
use rusqlite::types::{FromSql, FromSqlResult, ValueRef, FromSqlError};
use std::fmt::Display;
use failure::_core::fmt::Formatter;

pub type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;
pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

#[derive(Deserialize)]
struct AlbumsQuery {
    artist: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Artist {
    name: String
}

struct AlbumArtwork {
    album_id: i32,
    path: std::path::PathBuf
}

#[derive(Serialize, Deserialize, Debug)]
struct Album {
    id: i32,
    name: String,
    artist: Artist,
    year: Option<i32>
}

#[derive(Serialize, Debug)]
struct Response<T : Serialize> {
    values: Vec<T>
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct TrackId(String);

impl TrackId {
    fn new(value: String) -> Self {
        TrackId(base64_url::encode(&value))
    }
}

impl Display for TrackId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Serialize, Debug)]
struct Track {
    id: TrackId,
    name: String,
    album: Option<Album>,
    track_nr: Option<i32>,
    filename: String,
    #[serde(skip)]
    path: PathBuf
}

struct PathBlob(PathBuf);

impl PathBlob {
    // TODO: Def. do not use this
    fn cleanup_path(mut self) -> Self {
        if self.0.starts_with("/home/simao/MusicBeets") {
            let path_str = self.0.to_str().expect("Invalid Path").replace("/home/simao/MusicBeets", "/home/simao/MusicPi/Music");
            self.0 = PathBuf::from(&path_str);
            self
        } else {
            self
        }
    }
}

impl FromSql for PathBlob {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let path_blob = value.as_blob()?;
        let path_str = std::str::from_utf8(path_blob).map_err(|err| FromSqlError::Other(err.into()))?;
        Ok(PathBlob(PathBuf::from(path_str)).cleanup_path())
    }
}


fn find_album_tracks(c: &Connection, album_id: i32) -> Result<Vec<Track>, Error> {
    let mut stmt = c.prepare("select artist, a.album, title, track, path, a.year from items i join albums a on i.album = a.album where a.id = ?")?;

    let rows = stmt.query_map(params![album_id], |row| {
        let path: PathBlob = row.get("path")?;
        let filename = path.0.file_name().and_then(|s| s.to_str()).expect("Invalid track path").to_owned(); //TODO

        let album =
            Album {
                id: album_id,
                name: row.get("album")?,
                artist: Artist {
                    name: row.get("artist")?
                },
                year: row.get("year")?
            };

        Ok(
            Track {
                id: TrackId::new(row.get("title")?),
                name: row.get("title")?,
                album: Some(album),
                track_nr: row.get("track")?,
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

fn find_album_artwork(c: &Connection, id: i32) -> Result<AlbumArtwork, Error> {
    let mut stmt = c.prepare("select artpath FROM albums a join items i on i.album = a.album and i.artist_sort = a.albumartist_sort where a.id = ? limit 1")?;

    let row = stmt.query_row(params![id], |row| {
        let path: PathBlob = row.get("artpath")?;

        Ok(
            AlbumArtwork {
                album_id: id,
                path: path.0
            }
        )
    })?;

    Ok(row)
}

fn find_albums(c: &Connection, artist: Option<String>) -> Result<Vec<Album>, Error> {
    let parse_fn = |row: &rusqlite::Row| {
        Ok(
            Album {
                id: row.get(0)?,
                name: row.get(1)?,
                artist: Artist { name: row.get(2)? },
                year: row.get(3)?
            }
        )
    };

    let mut stmt;

    let rows = if let Some(a) = artist {
        stmt = c.prepare("SELECT id, album, albumartist, year FROM albums where albumartist = ?")?;
        stmt.query_map(params![a], parse_fn)?
    } else {
        stmt = c.prepare("SELECT id, album, albumartist, year FROM albums")?;
        stmt.query_map(params![], parse_fn)?
    };

    let mut res = vec![];

    for row in rows {
        res.push(row?);
    }

    Ok(res)
}

fn find_artists(c: &Connection) -> Result<Vec<Artist>, Error> {
    let mut stmt = c.prepare("SELECT distinct albumartist FROM albums")?;

    let rows = stmt.query_map(params![], |row| {
        Ok(
            Artist {
                name: row.get(0)?,
            }
        )
    })?;

    let mut res = vec![];

    for row in rows {
        res.push(row?);
    }

    Ok(res)
}

async fn list_albums(query: web::Query<AlbumsQuery>, pool: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let conn = pool.get()?;
    let resp = find_albums(&conn, query.artist.clone())?;
    Ok(HttpResponse::Ok().json(Response { values: resp }))
}

async fn get_album_artwork(path: web::Path<(i32, )>, pool: web::Data<Pool>) -> Result<NamedFile, Error> {
    let c = pool.get()?;
    let a = find_album_artwork(&c, path.0)?;
    let file = actix_files::NamedFile::open(a.path)?;
    Ok(file)
}

async fn get_album_tracks(path: web::Path<(i32, )>, pool: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let c = pool.get()?;
    let tracks = find_album_tracks(&c, path.0)?;
    Ok(HttpResponse::Ok().json(Response { values: tracks }))
}

async fn get_album_track_audio(req: HttpRequest, path: web::Path<(i32, TrackId, )>, pool: web::Data<Pool>) -> Result<HttpResponse, actix_web::Error> {
    let c = pool.get().map_err(failure::Error::from)?;
    let tracks = find_album_tracks(&c, path.0)?;

    if let Some(track) = tracks.iter().find(|t| t.id == path.1) {
        let file = actix_files::NamedFile::open(&track.path)?;
        actix_files::NamedFile::open(file.path())?.into_response(&req)
    } else {
        Ok(HttpResponse::NotFound().body(format!("Track {} for album {} not found", path.1, path.0)))
    }
}

async fn list_artists(pool: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let c = pool.get()?;
    let res = find_artists(&c)?;
    Ok(HttpResponse::Ok().json(Response { values: res }))
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let manager = SqliteConnectionManager::file("library.db");
    let pool = r2d2::Pool::new(manager).expect("could not open db");

    HttpServer::new(move || { App::new()
        .wrap(middleware::Logger::default())
        .data(pool.clone())
        .route("/albums", web::get().to(list_albums))
        .route("/albums/{id}/artwork", web::get().to(get_album_artwork))
        .route("/albums/{id}/tracks", web::get().to(get_album_tracks))
        .route("/albums/{id}/tracks/{track_id}/audio", web::get().to(get_album_track_audio))
        .route("/artists", web::get().to(list_artists))
    }).bind("0.0.0.0:3030")
        .unwrap()
        .run()
        .await
}
