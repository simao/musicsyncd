use structopt::StructOpt;
use std::path::PathBuf;

#[derive(StructOpt, Debug)]
#[structopt(name = "musicsyncd")]
pub struct ConfigOpt {
    /// Directory where to save music database. Defaults to using your system's cache directory
    #[structopt(short, long, parse(from_os_str))]
    pub cache_dir: Option<PathBuf>,

    /// Music Directory where files are organized by Artist/Album/Tracks
    #[structopt(short, long, parse(from_os_str))]
    pub music_dir: PathBuf,

    /// Do not scan music directory on startup, just use the db in `cache_dir` as is
    #[structopt(long = "no-reload", parse(from_flag = std::ops::Not::not))]
    pub reload: bool,


    /// Bind port
    #[structopt(short, long, default_value="3030")]
    pub port: u16
}

