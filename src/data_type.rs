use serde::{Serialize, Deserialize};
use std::path::{PathBuf};

#[derive(Serialize, Deserialize, Debug)]
pub struct Artist {
    pub id: ArtistId,
    pub name: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArtistId(pub i64);

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct AlbumId(pub i64);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct TrackId(pub i64);

pub struct AlbumArtwork {
    pub album_id: i64,
    pub path: PathBuf
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Album {
    pub id: AlbumId,
    pub title: String,
    pub artist: Artist,
}


#[derive(Serialize, Debug)]
pub struct FullAlbum {
    pub album: Album,
    pub tracks: Vec<Track>
}

#[derive(Serialize, Debug)]
pub struct Response<T : Serialize> {
    pub values: Vec<T>
}

#[derive(Serialize, Debug)]
pub struct Track {
    pub id: TrackId,
    pub title: Option<String>,
    pub filename: String,
    #[serde(skip)]
    pub path: PathBuf
}

pub struct PathBlob(pub PathBuf);
