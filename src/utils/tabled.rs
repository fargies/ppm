/*
** Copyright (C) 2026 Sylvain Fargier
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
** Created on: 2026-01-03T23:44:57
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use colored::Colorize;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local, SecondsFormat};
use humantime::format_duration;

use crate::utils::IS_OUT_COLORED;

pub trait TabledDisplay {
    fn to_string(&self) -> String;
}

impl TabledDisplay for Duration {
    fn to_string(&self) -> String {
        format_duration(*self).to_string()
    }
}

impl TabledDisplay for SystemTime {
    fn to_string(&self) -> String {
        DateTime::<Local>::from(*self).to_rfc3339_opts(SecondsFormat::Secs, true)
    }
}

impl TabledDisplay for bool {
    fn to_string(&self) -> String {
        if IS_OUT_COLORED.get() {
            if *self {
                String::from("\u{2714}").green().to_string()
            } else {
                String::from("\u{2718}").red().to_string()
            }
        } else {
            ToString::to_string(self)
        }
    }
}

impl<T> TabledDisplay for Option<T>
where
    T: TabledDisplay,
{
    fn to_string(&self) -> String {
        self.as_ref().map_or_else(String::new, |t| t.to_string())
    }
}
