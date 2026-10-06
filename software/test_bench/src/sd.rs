use std::{
    error::Error,
    fmt, fs,
    io::{Read, Write},
    num::NonZeroUsize,
    ops::ControlFlow,
    path::{Path},
    thread::{Scope, ScopedJoinHandle},
    sync::Arc,
};

use crate::{
    active_object::{ActiveObject, Address, Mailbox},
    screen::ScreenMessage,
    shared_types::Song,
};
use esp_idf_svc::{
    fs::fatfs::Fatfs,
    hal::{
        gpio::{AnyIOPin, Gpio13},
        sd::{spi::SdSpiHostDriver, SdCardConfiguration, SdCardDriver},
        spi::SpiDriver,
    },
    io::vfs::MountedFatfs,
    sys::EspError,
};
use log::info;

const MOUNT: &str = "/sdcard";
pub const MP3_DIR: &str = "/sdcard/mp3_samples/";
const TEST_FILE: &str = "/sdcard/JPODTEST.TXT";
const TEST_DATA: &[u8] = b"jpod sd card test";

#[derive(Debug)]
pub enum SdError {
    Esp(EspError),
    Io(std::io::Error),
    ReadBackMismatch,
}

impl From<EspError> for SdError {
    fn from(error: EspError) -> Self {
        Self::Esp(error)
    }
}

impl From<std::io::Error> for SdError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for SdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Esp(error) => write!(formatter, "ESP-IDF error: {error}"),
            Self::Io(error) => write!(formatter, "filesystem error: {error}"),
            Self::ReadBackMismatch => write!(formatter, "SD read-back mismatch"),
        }
    }
}

impl Error for SdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Esp(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::ReadBackMismatch => None,
        }
    }
}

pub type SdWorker<'scope> = ScopedJoinHandle<'scope, Result<(), SdError>>;

pub enum SdMessage {
    FetchSongs(Address<ScreenMessage>),
}

struct SdFilesystem<MountGuard> {
    _mount: MountGuard,
    songs: Arc<Vec<Song>>,
}

impl<MountGuard> SdFilesystem<MountGuard> {
    pub fn list_files_recursive(&self, directory: impl AsRef<Path>) -> Result<(), SdError> {
        list_directory(directory.as_ref())
    }

    fn handle_message(&mut self, message: SdMessage) -> ControlFlow<()> {
        match message {
            SdMessage::FetchSongs(screen_address) => {
                if screen_address
                    .post(ScreenMessage::ShareSongs(Arc::clone(&self.songs)))
                    .is_err()
                {
                    log::error!("sd: screen worker disconnected");
                    return ControlFlow::Break(());
                }
            }
        }
        ControlFlow::Continue(())
    }
}

pub fn spawn_sd_worker<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    mailbox: Mailbox<SdMessage>,
    bus: &'env SpiDriver<'env>,
    cs: Gpio13<'env>,
) -> SdWorker<'scope> {
    ActiveObject::new("sd", NonZeroUsize::new(8 * 1024).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || initialize_sd(bus, cs),
            SdFilesystem::handle_message,
        )
        .unwrap_or_else(|_| crate::restart_on_failure("Failed to start SD worker"))
}

fn initialize_sd<'spi>(
    bus: &'spi SpiDriver<'spi>,
    cs: Gpio13<'spi>,
) -> Result<SdFilesystem<impl Sized + 'spi>, SdError> {
    let host = SdSpiHostDriver::new(
        bus,
        Some(cs),
        AnyIOPin::none(),
        AnyIOPin::none(),
        AnyIOPin::none(),
        None,
    )?;
    let card = SdCardDriver::new_spi(host, &SdCardConfiguration::new())?;
    let mounted_fs = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT, 4)?;
    info!("sd: mounted at {MOUNT}");

    verify_file_operations()?;
    let songs: Arc<Vec<Song>> = Arc::new(collect_songs(MP3_DIR)?);
    Ok(SdFilesystem {
        _mount: mounted_fs,
        songs,
    })
}

fn collect_songs(directory: impl AsRef<Path>) -> Result<Vec<Song>, SdError> {
    let mut songs = Vec::new();

    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_file() && is_mp3(&path) {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            info!("sd: song path={path:?}, display name={name:?}");
            songs.push(Song { path, name });
        }
    }

    Ok(songs)
}

fn verify_file_operations() -> Result<(), SdError> {
    fs::File::create(TEST_FILE)?.write_all(TEST_DATA)?;

    let mut read_back = Vec::new();
    fs::File::open(TEST_FILE)?.read_to_end(&mut read_back)?;
    if read_back != TEST_DATA {
        fs::remove_file(TEST_FILE)?;
        return Err(SdError::ReadBackMismatch);
    }
    fs::remove_file(TEST_FILE)?;
    info!("sd: write + read-back + delete OK");

    Ok(())
}

fn list_directory(directory: &Path) -> Result<(), SdError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            list_directory(&path)?;
        } else if entry.file_type()?.is_file() {
            info!("sd:   {}", path.display());
        }
    }

    Ok(())
}

fn is_mp3(path: &Path) -> bool {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => extension.eq_ignore_ascii_case("mp3"),
        None => false,
    }
}
