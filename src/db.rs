
use crate::data_type::*;
use crate::error::*;

use rusqlite::types::{FromSql, FromSqlResult, ValueRef};
use rusqlite::types::{ToSqlOutput};
use rusqlite::{params, ToSql};
use rusqlite::NO_PARAMS;
use anyhow::Error;
use std::path::PathBuf;
use audiotags::Tag;


pub type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;
pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

impl ToSql for ArtistId {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        Ok(ToSqlOutput::from(self.0))
    }
}

impl FromSql for ArtistId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_i64().map(Self)
    }
}

impl ToSql for AlbumId {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        Ok(ToSqlOutput::from(self.0))
    }
}

impl FromSql for AlbumId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_i64().map(Self)
    }
}

impl FromSql for PathBlob {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let path_str = value.as_str()?;
        Ok(PathBlob(std::path::PathBuf::from(path_str)))
    }
}

impl ToSql for PathBlob {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self.0.to_str() {
            None => Ok(ToSqlOutput::from(rusqlite::types::Null)),
            Some(s) => Ok(ToSqlOutput::from(s.to_owned()))
        }
    }
}

pub fn clean_db(c: &Connection) -> Result<(), Error> {
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
pub fn reload_db(base_dir: &PathBuf, c: &mut Connection) -> Result<(), Error> {
    log::info!("Reloading db using {}", base_dir.display());

    let paths = std::fs::read_dir(&base_dir)?;

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
        let mut dir = base_dir.clone();
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

            tx.execute("INSERT INTO albums (title, artist_id, art_path) VALUES (?1, ?2, ?3)",
                       params![name, aid, db_artwork_path])?;

            let album_id = tx.last_insert_rowid();

            if ! apath.is_dir() {
                continue;
            }

            log::debug!("Searching for tracks in {:?}", apath);

            let track_paths = std::fs::read_dir(apath)?;

            for tentry in track_paths {
                let tpath = tentry?.path();

                if ! tpath.is_file() || tpath.extension().unwrap() == "jpg" { // TODO: more cover types
                    continue;
                }

                log::debug!("Reading tags for {:?}", tpath);

                let t = Tag::default().read_from_path(&tpath);

                match t {
                    Ok(audio_tags) => {
                        tx.execute("INSERT INTO tracks (title, album_id, full_path) VALUES (?1, ?2, ?3)",
                                   params![audio_tags.title(), album_id, PathBlob(tpath)])?;
                        ()
                    },
                    Err(err) =>
                        log::debug!("Could not read metadata from {:?}: {:?}", tpath, err)
                };
            }

        }
    }

    tx.commit()?;

    log::info!("Finished reloading db");

    Ok(())
}


pub fn find_artist_full_albums(c: &Connection, artist_id: ArtistId) -> Result<Vec<FullAlbum>, MyError>{
    let mut artist_albums = find_albums(&c, Some(artist_id))?;

    let mut res: Vec<FullAlbum> = vec![];

    for album in artist_albums.drain(0..) {
        let tracks = find_album_tracks(&c, album.id)?;

        let fa = FullAlbum { album, tracks };
        res.push(fa)
    }

    Ok(res)
}

pub fn find_album_tracks(c: &Connection, album_id: AlbumId) -> Result<Vec<Track>, MyError> {
    let mut stmt = c.prepare(
        "select id, title, full_path from tracks where album_id = ?")?;

    let rows = stmt.query_map(params![album_id], |row| {
        let path: PathBlob = row.get("full_path")?;
        let filename = path.0.file_name().expect("invalid track path").to_string_lossy().to_string();

        Ok(
            Track {
                id: TrackId(row.get("id")?),
                title: row.get("title")?,
                path: path.0,
                filename
            }
        )
    })?;

    let res: Result<Vec<_>, _>= rows.collect();
    Ok(res?)
}


pub fn find_album_artwork(c: &Connection, album_id: i64) -> Result<AlbumArtwork, Error> {
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

fn find_albums(c: &Connection, artist_id: Option<ArtistId>) -> Result<Vec<Album>, Error> {
    let mut sql = "\
      SELECT at.id artist_id, at.name, al.id album_id, al.title \
      from artists at INNER JOIN albums al \
      on al.artist_id = at.id".to_owned();

    let aid: ArtistId;
    let mut params: Vec<&dyn ToSql> = vec![];

    if let Some(_aid) = artist_id {
        aid = _aid;
        sql = sql + " WHERE artist_id = ?";
        params.push(&aid.0);
    }

    let mut stmt = c.prepare(&sql)?;

    let rows = stmt.query_map(params, |row: &rusqlite::Row| {
        let artist_id: ArtistId = row.get("artist_id")?;
        let name: String = row.get("name")?;
        let album_id: AlbumId = row.get("album_id")?;
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

pub fn find_artists(c: &Connection) -> Result<Vec<Artist>, Error> {
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
