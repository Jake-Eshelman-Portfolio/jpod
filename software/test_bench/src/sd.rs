use std::{
    error::Error,
    fmt, fs,
    ops::ControlFlow,
    path::{Path},
    thread::ScopedJoinHandle,
    sync::Arc,
};
#[cfg(target_os = "espidf")]
use std::{io::{Read, Write}, num::NonZeroUsize, thread::Scope};

use crate::{
    active_object::Address,
    dac::DacMessage,
    screen::ScreenMessage,
    shared_types::Song,
};
#[cfg(target_os = "espidf")]
use crate::active_object::{ActiveObject, Mailbox};
#[cfg(target_os = "espidf")]
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

use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions
};

#[cfg(target_os = "espidf")]
const MOUNT: &str = "/sdcard";
pub const MP3_DIR: &str = "/sdcard/mp3_samples/";
#[cfg(target_os = "espidf")]
const TEST_FILE: &str = "/sdcard/JPODTEST.TXT";
#[cfg(target_os = "espidf")]
const TEST_DATA: &[u8] = b"jpod sd card test";

#[derive(Debug)]
pub enum SdError {
    #[cfg(target_os = "espidf")]
    Esp(EspError),
    Io(std::io::Error),
    ReadBackMismatch,
}

#[cfg(target_os = "espidf")]
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
            #[cfg(target_os = "espidf")]
            Self::Esp(error) => write!(formatter, "ESP-IDF error: {error}"),
            Self::Io(error) => write!(formatter, "filesystem error: {error}"),
            Self::ReadBackMismatch => write!(formatter, "SD read-back mismatch"),
        }
    }
}

impl Error for SdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            #[cfg(target_os = "espidf")]
            Self::Esp(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::ReadBackMismatch => None,
        }
    }
}

pub type SdWorker<'scope> = ScopedJoinHandle<'scope, Result<(), SdError>>;

pub enum SdMessage {
    FetchSongs(Address<ScreenMessage>),
    DecodeMP3(usize),
}

pub struct SdFilesystem<MountGuard> {
    _mount: MountGuard,
    songs: Arc<Vec<Song>>,
    dac_address: Address<DacMessage>,
}

impl<MountGuard> SdFilesystem<MountGuard> {
    pub fn new(
        mount: MountGuard,
        directory: impl AsRef<Path>,
        dac_address: Address<DacMessage>,
    ) -> Result<Self, SdError> {
        Ok(Self {
            _mount: mount,
            songs: Arc::new(collect_songs(directory)?),
            dac_address,
        })
    }

    pub fn list_files_recursive(&self, directory: impl AsRef<Path>) -> Result<(), SdError> {
        list_directory(directory.as_ref())
    }

    pub fn handle_message(&mut self, message: SdMessage) -> ControlFlow<()> {
        match message {
            SdMessage::FetchSongs(screen_address) => {
                if screen_address
                // Post (address, message)
                    .post(ScreenMessage::ShareSongs(Arc::clone(&self.songs)))
                    .is_err()
                {
                    log::error!("sd: screen worker disconnected");
                    return ControlFlow::Break(());
                }
            }
            SdMessage::DecodeMP3(song_position) => {
                decode_to_pcm_and_post(&self.songs[song_position].path, &self.dac_address);
                
                // if self.dac_address.try_post(DacMessage::PlaySong(song_position)).is_err() {
                //     log::error!("sd: DAC worker mailbox is unavailable");
                // }
            }
        }
        ControlFlow::Continue(())
    }
}

#[cfg(target_os = "espidf")]
pub fn spawn_sd_worker<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    mailbox: Mailbox<SdMessage>,
    bus: &'env SpiDriver<'env>,
    cs: Gpio13<'env>,
    dac_address: Address<DacMessage>,
) -> SdWorker<'scope> {
    ActiveObject::new("sd", NonZeroUsize::new(64 * 1024).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || initialize_sd(bus, cs, dac_address),
            SdFilesystem::handle_message,
        )
        .unwrap_or_else(|_| crate::restart_on_failure("Failed to start SD worker"))
}

#[cfg(target_os = "espidf")]
fn initialize_sd<'spi>(
    bus: &'spi SpiDriver<'spi>,
    cs: Gpio13<'spi>,
    dac_address: Address<DacMessage>,
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
    SdFilesystem::new(mounted_fs, MP3_DIR, dac_address)
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

#[cfg(target_os = "espidf")]
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



// Sample
fn decode_to_pcm_and_post(path: &Path, dac_address: &Address<DacMessage>) {
    // Open the media source.
    let src = std::fs::File::open(path).expect("failed to open media");

    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Create a probe hint using the file's extension. [Optional]
    let mut hint = Hint::new();
    hint.with_extension("mp3");

    // Use the default options for metadata and format readers.
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    // Probe the media source.
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .expect("unsupported format");

    // Find the first audio track with a known (decodeable) codec.
    let track = format.default_track(TrackType::Audio).expect("no audio track");

    // Use the default options for the decoder.
    let dec_opts: AudioDecoderOptions = Default::default();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(
            track.codec_params.as_ref().expect("codec parameters missing").audio().unwrap(),
            &dec_opts,
        )
        .expect("unsupported codec");

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => {
                // Reached the end of the stream.
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(err) => {
                // A unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let mut samples = Vec::<i16>::new();
                decoded.copy_to_vec_interleaved(&mut samples);
                dac_address.post(DacMessage::PlayChunk(samples));
            }
            Err(SymphoniaError::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                continue;
            }
            Err(SymphoniaError::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                continue;
            }
            Err(err) => {
                // An unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        }
    }
}
