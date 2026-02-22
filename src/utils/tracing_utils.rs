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

#![allow(dead_code)]

use std::io::IsTerminal;

pub fn is_log_color<T>(output: &T) -> bool
where
    T: IsTerminal,
{
    match std::env::var("RUST_LOG_STYLE")
        .unwrap_or_else(|_| String::from("auto"))
        .to_lowercase()
        .as_str()
    {
        "never" | "no" | "0" => false,
        "always" | "yes" | "1" => true,
        _ => output.is_terminal(),
    }
}
