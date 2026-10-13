// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import { DateTime, Duration, Interval } from "luxon";

export function isNil(value) {
  return value === null || typeof value === "undefined";
}

export function formatDate(date) {
  return date?.toLocaleString(DateTime.DATETIME_SHORT_WITH_SECONDS);
}

export function formatDuration(duration) {
  let millis = duration?.toMillis();
  if (millis >= 5000) {
    return Duration.fromMillis(millis - (millis % 1000))
      .rescale()
      .toHuman({ showZeros: false, unitDisplay: "narrow" })
      .replaceAll(",", "");
  } else if (millis) {
    return duration
      .shiftTo("seconds", "milliseconds")
      .removeZeros()
      .toHuman({ showZeros: false, unitDisplay: "narrow" })
      .replaceAll(",", "");
  }
}

export function formatBytes(value) {
  if (value >= 0x4000_0000) {
    return `${(value / 0x4000_0000).toFixed(2)} GiB`;
  } else if (value >= 0x10_0000) {
    return `${(value / 0x10_0000).toFixed(2)} MiB`;
  } else if (value >= 0x400) {
    return `${(value / 0x400).toFixed(2)} KiB`;
  } else if (value >= 0) {
    return `${value.toFixed(2)} B`;
  }
}

export function upTime(start_time, end_time) {
  let duration =
    start_time && end_time
      ? Interval.fromDateTimes(start_time, end_time)
      : null;
  return formatDuration(duration?.toDuration());
}

export function mapStatusBadge(status) {
  return (
    {
      running: "success",
      crashed: "failure",
    }[status?.toLowerCase()] ?? ""
  );
}
