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
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::sync::Arc;

use crate::utils::tracing_utils::TracingDateTime;

pub struct AutoDate {
    /// Feature is enabled/disabled or in "detect" mode
    enabled: Option<bool>,
    /// Will inject on next line
    next: Option<()>,
}

impl Default for AutoDate {
    fn default() -> Self {
        Self {
            enabled: None,
            next: Some(()),
        }
    }
}

impl AutoDate {
    pub fn take(&mut self, line: &[u8]) -> Option<Arc<String>>
    {
        if self.next.take().is_some() {
            match self.enabled {
                Some(true) => Some(TracingDateTime::get()),
                Some(false) => None,
                None => {
                    let enabled = !TracingDateTime::has_date_time(line);
                    self.enabled = Some(enabled);
                    enabled.then(TracingDateTime::get)
                }
            }
        } else {
            None
        }
    }

    pub fn prepare<'a>(&mut self, buffer: &'a [u8]) -> &'a [u8] {
        match buffer.iter().position(|&c| c == b'\n') {
            Some(pos) => {
                self.next = Some(());
                &buffer[..pos + 1]
            },
            None => buffer,
        }
    }

    pub fn disable(&mut self) {
        self.enabled = Some(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_date() {
        let mut ad = AutoDate::default();

        // date/time missing --> detect and inject
        assert!(ad.take(b"test").is_some());
        assert_eq!(ad.prepare(b"test\n"), b"test\n");
        // test is made only on first call
        assert!(ad.take(TracingDateTime::get().as_bytes()).is_some());

        let mut ad = AutoDate::default();
        assert!(ad.take(TracingDateTime::get().as_bytes()).is_none());
        assert_eq!(ad.prepare(b"test\n"), b"test\n");
        // test is made only on first call
        assert!(ad.take(b"test").is_none());
    }
}
