use std::path::{Path, PathBuf};

/// A sound file that could not be read or decoded.
#[derive(Debug)]
pub struct LoadError {
    path: PathBuf,
    cause: Cause,
}

#[derive(Debug)]
enum Cause {
    Io(std::io::Error),
    #[cfg(feature = "rustysynth")]
    SoundFont(rustysynth::SoundFontError),
    #[cfg(feature = "symphonia")]
    Decode(symphonia::core::errors::Error),
    #[cfg(feature = "symphonia")]
    NoAudio,
}

impl LoadError {
    /// The file that failed to load.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn io(path: &Path, error: std::io::Error) -> Self {
        Self {
            path: path.to_owned(),
            cause: Cause::Io(error),
        }
    }

    #[cfg(feature = "rustysynth")]
    pub(crate) fn soundfont(
        path: &Path,
        error: rustysynth::SoundFontError,
    ) -> Self {
        Self {
            path: path.to_owned(),
            cause: Cause::SoundFont(error),
        }
    }

    #[cfg(feature = "symphonia")]
    pub(crate) fn decode(
        path: &Path,
        error: symphonia::core::errors::Error,
    ) -> Self {
        Self {
            path: path.to_owned(),
            cause: Cause::Decode(error),
        }
    }

    #[cfg(feature = "symphonia")]
    pub(crate) fn no_audio(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            cause: Cause::NoAudio,
        }
    }
}

impl std::fmt::Display for LoadError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "could not load {}: ", self.path.display())?;
        match &self.cause {
            Cause::Io(error) => error.fmt(f),
            #[cfg(feature = "rustysynth")]
            Cause::SoundFont(error) => error.fmt(f),
            #[cfg(feature = "symphonia")]
            Cause::Decode(error) => error.fmt(f),
            #[cfg(feature = "symphonia")]
            Cause::NoAudio => {
                f.write_str("the file has no audio track")
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.cause {
            Cause::Io(error) => Some(error),
            #[cfg(feature = "rustysynth")]
            Cause::SoundFont(error) => Some(error),
            #[cfg(feature = "symphonia")]
            Cause::Decode(error) => Some(error),
            #[cfg(feature = "symphonia")]
            Cause::NoAudio => None,
        }
    }
}
