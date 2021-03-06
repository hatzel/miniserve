use actix_web::http::ContentEncoding;
use bytes::Bytes;
use failure::ResultExt;
use libflate::gzip::Encoder;
use serde::Deserialize;
use std::io;
use std::path::PathBuf;
use strum_macros::{Display, EnumIter, EnumString};
use tar::Builder;

use crate::errors;

/// Available compression methods
#[derive(Deserialize, Clone, EnumIter, EnumString, Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CompressionMethod {
    /// TAR GZ
    TarGz,
}

impl CompressionMethod {
    pub fn extension(&self) -> String {
        match &self {
            CompressionMethod::TarGz => "tar.gz",
        }
        .to_string()
    }

    pub fn content_type(&self) -> String {
        match &self {
            CompressionMethod::TarGz => "application/gzip",
        }
        .to_string()
    }

    pub fn content_encoding(&self) -> ContentEncoding {
        match &self {
            CompressionMethod::TarGz => ContentEncoding::Gzip,
        }
    }
}

/// Creates an archive of a folder, using the algorithm the user chose from the web interface
/// This method returns the archive as a stream of bytes
pub fn create_archive(
    method: &CompressionMethod,
    dir: &PathBuf,
    skip_symlinks: bool,
) -> Result<(String, Bytes), errors::CompressionError> {
    match method {
        CompressionMethod::TarGz => tgz_compress(&dir, skip_symlinks),
    }
}

/// Compresses a given folder in .tar.gz format, and returns the result as a stream of bytes
fn tgz_compress(
    dir: &PathBuf,
    skip_symlinks: bool,
) -> Result<(String, Bytes), errors::CompressionError> {
    let src_dir = dir.display().to_string();
    let inner_folder = match dir.file_name() {
        Some(directory_name) => match directory_name.to_str() {
            Some(directory) => directory,
            None => {
                return Err(errors::CompressionError::new(
                    errors::CompressionErrorKind::InvalidUTF8DirectoryName,
                ))
            }
        },
        None => {
            return Err(errors::CompressionError::new(
                errors::CompressionErrorKind::InvalidDirectoryName,
            ))
        }
    };
    let dst_filename = format!("{}.tar", inner_folder);
    let dst_tgz_filename = format!("{}.gz", dst_filename);

    let tar_content = tar(src_dir, inner_folder.to_string(), skip_symlinks).context(
        errors::CompressionErrorKind::TarBuildingError {
            message: "an error occured while writing the TAR archive".to_string(),
        },
    )?;
    let gz_data = gzip(&tar_content).context(errors::CompressionErrorKind::GZipBuildingError {
        message: "an error occured while writing the GZIP archive".to_string(),
    })?;

    let mut data = Bytes::new();
    data.extend_from_slice(&gz_data);

    Ok((dst_tgz_filename, data))
}

/// Creates a TAR archive of a folder, and returns it as a stream of bytes
fn tar(
    src_dir: String,
    inner_folder: String,
    skip_symlinks: bool,
) -> Result<Vec<u8>, errors::CompressionError> {
    let mut tar_builder = Builder::new(Vec::new());

    tar_builder.follow_symlinks(!skip_symlinks);
    // Recursively adds the content of src_dir into the archive stream
    tar_builder.append_dir_all(inner_folder, &src_dir).context(
        errors::CompressionErrorKind::TarBuildingError {
            message: format!(
                "failed to append the content of {} to the TAR archive",
                &src_dir
            ),
        },
    )?;

    let tar_content =
        tar_builder
            .into_inner()
            .context(errors::CompressionErrorKind::TarBuildingError {
                message: "failed to finish writing the TAR archive".to_string(),
            })?;

    Ok(tar_content)
}

/// Compresses a stream of bytes using the GZIP algorithm, and returns the resulting stream
fn gzip(mut data: &[u8]) -> Result<Vec<u8>, errors::CompressionError> {
    let mut encoder =
        Encoder::new(Vec::new()).context(errors::CompressionErrorKind::GZipBuildingError {
            message: "failed to create GZIP encoder".to_string(),
        })?;
    io::copy(&mut data, &mut encoder).context(errors::CompressionErrorKind::GZipBuildingError {
        message: "failed to write GZIP data".to_string(),
    })?;
    let data = encoder.finish().into_result().context(
        errors::CompressionErrorKind::GZipBuildingError {
            message: "failed to write GZIP trailer".to_string(),
        },
    )?;

    Ok(data)
}
