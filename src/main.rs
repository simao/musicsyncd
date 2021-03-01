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

use katatsuki::{Track as KTrack};


#[derive(Deserialize)]
struct FullAlbumsQuery {
    artist: String,
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
struct FullAlbum {
    album: Album,
    tracks: Vec<Track>
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
    track_nr: Option<i32>,
    filename: String,
    #[serde(skip)]
    path: PathBuf
}

struct PathBlob(PathBuf);

impl FromSql for PathBlob {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let path_blob = value.as_blob()?;
        let path_str = std::str::from_utf8(path_blob).map_err(|err| FromSqlError::Other(err.into()))?;
        Ok(PathBlob(PathBuf::from(path_str)))
    }
}

fn find_artist_full_albums(c: &Connection, base_dir: &BaseDir, artist_name: String) -> Result<Vec<FullAlbum>, Error>{
    let mut artist_albums = find_albums(&base_dir, Some(artist_name))?;

    let mut res: Vec<FullAlbum> = vec![];

    for album in artist_albums.drain(0..) {
        let tracks = find_album_tracks(&c, album.id)?;

        let fa = FullAlbum { album, tracks };
        res.push(fa)
    }

    Ok(res)
}

fn find_album_tracks(c: &Connection, album_id: i32) -> Result<Vec<Track>, Error> {
    let mut stmt = c.prepare("select artist, a.album, title, track, path, a.year from items i join albums a on i.album = a.album where a.id = ?")?;

    let rows = stmt.query_map(params![album_id], |row| {
        let path: PathBlob = row.get("path")?;
        let filename = path.0.file_name().and_then(|s| s.to_str()).expect("Invalid track path").to_owned(); //TODO

        Ok(
            Track {
                id: TrackId::new(row.get("title")?),
                name: row.get("title")?,
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

fn find_albums(base_dir: &BaseDir, artist: Option<String>) -> Result<Vec<Album>, Error> {

    // TODO: Fix unwrap, just find all albums

    let mut dir = base_dir.0.clone();
    dir.push(PathBuf::from(artist.unwrap()));

    let album_dirs = std::fs::read_dir(dir)?;

    let mut res: Vec<Album> = vec![];

    for adir in album_dirs {
        let apath = adir?.path();

        let first_track = std::fs::read_dir(&apath)?.nth(0).unwrap()?; // TODO: Unwrap

        let ktrack = KTrack::from_path(&first_track.path(), None)?; // TODO: Use None on metadata if this fails, it will fail a lot I suppose, often with unpredicatble effects because of for example "year"

        let album = Album {
            id: 0, // TODO: What
            name: apath.file_name().unwrap().to_string_lossy().into(),
            artist: Artist { name: ktrack.artist },
            year: Some(ktrack.year)
        };

        res.push(album);
    }

    Ok(res)
}

fn find_artists(base_dir: &BaseDir) -> Result<Vec<Artist>, Error> {
    let paths = std::fs::read_dir(&base_dir.0)?;

    let mut res: Vec<Artist> = vec![];

    for path in paths {
        res.push({
            Artist {
                name: format!("{}", path?.file_name().to_string_lossy()),
            }
        })
    }

    Ok(res)
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

async fn get_artist_full_albums(query: web::Query<FullAlbumsQuery>, base_dir: web::Data<BaseDir>, pool: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let conn = pool.get()?;
    let resp = find_artist_full_albums(&conn, &base_dir, query.artist.clone())?;
    Ok(HttpResponse::Ok().json(Response { values: resp }))
}


async fn list_artists(base_dir: web::Data<BaseDir>) -> Result<HttpResponse, Error> {
    let res = find_artists(&base_dir)?;
    Ok(HttpResponse::Ok().json(Response { values: res }))
}

#[derive(Clone, Debug)]
struct BaseDir(PathBuf);

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let manager = SqliteConnectionManager::file("library.db");
    let pool = r2d2::Pool::new(manager).expect("could not open db");

    let base_dir = BaseDir(PathBuf::from("/home/simao/MusicBeets"));

    HttpServer::new(move || { App::new()
        .wrap(middleware::Logger::default())
        .data(pool.clone())
        .data(base_dir.clone())
        .route("/full-albums", web::get().to(get_artist_full_albums))
        .route("/albums/{id}/artwork", web::get().to(get_album_artwork))
        .route("/albums/{id}/tracks", web::get().to(get_album_tracks))
        .route("/albums/{id}/tracks/{track_id}/audio", web::get().to(get_album_track_audio))
        .route("/artists", web::get().to(list_artists))

    }).bind("0.0.0.0:3030")
        .unwrap()
        .run()
        .await
}
