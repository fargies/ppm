/*
** Copyright (C) 2025 Sylvain Fargier
**
** This software is provided 'as-is', without any express or implied
** warranty.  In no event will the authors be held liable for any damages
** arising from the use of this software.
**
** Permission is granted to anyone to use this software for any purpose,
** including commercial applications, and to alter it and redistribute it
** freely, subject to the following restrictions:
**
** 1. The origin of this software must not be misrepresented; you must not
**    claim that you wrote the original software. If you use this software
**    in a product, an acknowledgment in the product documentation would be
**    appreciated but is not required.
** 2. Altered source versions must be plainly marked as such, and must not be
**    misrepresented as being the original software.
** 3. This notice may not be removed or altered from any source distribution.
**
** Created on: 2025-09-22T15:34:13
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

#![allow(dead_code)]

use std::{
    env::temp_dir,
    fs::{File, create_dir},
    io::{ErrorKind, Result},
    ops::{Deref, DerefMut},
    path::PathBuf,
};

/// Convenience empty struct
///
/// See [MkTemp::file] and [MkTemp::dir]
pub struct MkTemp();

/// A temporary file
pub struct TempFile {
    path: PathBuf,
    file: Option<File>,
}

impl AsRef<PathBuf> for TempFile {
    fn as_ref(&self) -> &PathBuf {
        &self.path
    }
}

impl AsRef<File> for TempFile {
    fn as_ref(&self) -> &File {
        self.file.as_ref().unwrap()
    }
}

impl AsMut<File> for TempFile {
    fn as_mut(&mut self) -> &mut File {
        self.file.as_mut().unwrap()
    }
}

impl Deref for TempFile {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        self.file.as_ref().unwrap()
    }
}

impl DerefMut for TempFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.file.as_mut().unwrap()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        self.file = None;
        std::fs::remove_file(&self.path).expect("failed to remove temporary file: {self.path}");
    }
}

/// A temporary directory
pub struct TempDir {
    path: PathBuf,
}

impl AsRef<PathBuf> for TempDir {
    fn as_ref(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).expect("failed to remove temporary dir: {self.path}");
    }
}

impl MkTemp {
    /// Create a temporary file
    ///
    /// The temporary file is deleted when object is dropped
    pub fn file(prefix: &str) -> Result<TempFile> {
        let temp_dir = temp_dir();
        let mut suffix = 0;
        loop {
            let path = temp_dir.join(format!("{prefix}-{suffix}"));
            match File::create_new(&path) {
                Ok(file) => {
                    return Ok(TempFile {
                        path,
                        file: Some(file),
                    });
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => suffix += 1,
                Err(err) => panic!("failed to create temporary file: {err}"),
            }
        }
    }

    /// Create a temporary directory
    ///
    /// The temporary directorys is deleted when object is dropped
    pub fn dir(prefix: &str) -> Result<TempDir> {
        let temp_dir = temp_dir();
        let mut suffix = 0;
        loop {
            let path = temp_dir.join(format!("{prefix}-{suffix}"));
            match create_dir(&path) {
                Ok(_) => return Ok(TempDir { path }),
                Err(err) if err.kind() == ErrorKind::AlreadyExists => suffix += 1,
                Err(err) => panic!("failed to create temporary dir: {err}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_file() {
        let path: PathBuf = {
            let file = MkTemp::file("test").unwrap();
            let path: &PathBuf = file.as_ref();

            assert!(path.exists());
            assert!(!path.is_dir());
            path.clone()
        };
        assert!(!path.exists());
    }

    #[test]
    fn temp_dir() {
        let path: PathBuf = {
            let file = MkTemp::dir("test").unwrap();
            let path: &PathBuf = file.as_ref();

            assert!(path.exists());
            assert!(path.is_dir());
            path.clone()
        };
        assert!(!path.exists());
    }
}
