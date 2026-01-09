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
** Created on: 2025-12-24T10:29:15
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Status {
    /// initial status, before being spawned
    Created,
    /// process is running
    Running,
    /// process has finished with a `0` status code
    Finished,
    /// process is alive but stopped
    Stopped,
    /// process has finished with a `!= 0` status code
    Crashed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde() {
        assert_eq!(
            "Running\n",
            serde_yaml_ng::to_string(&Status::Running).unwrap()
        );
        assert_eq!(
            serde_yaml_ng::from_str::<Status>("Running").unwrap(),
            Status::Running
        );
    }
}
