// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import { useWebSocket } from "@vueuse/core";
import { DateTime, Duration } from "luxon";
import debug from "debug";

import { servicesInfo, serviceNameMap, servicesStats, addError } from "./store";
import { isNil } from './utils';

export let api_url;
if (typeof __API_URL__ === "undefined") {
  let url = new URL("ws", window.location.href);
  url.protocol.replace("http", "ws");
  api_url = url.href;
} else {
  api_url = __API_URL__;
}

var log = debug("client");

export class Client {
  ws;
  status = "UNKNOWN";
  /** @type {null|async (data: any)=>any} */
  onMessage = null;

  constructor(api_url) {
    log("client created");
    const { status, data, send, open, close, ws } = useWebSocket(api_url, {
      autoReconnect: { delay: 1000 },
      autoClose: false,
      onError(err) {
        addError(err?.message ?? "server connection closed");
      },
      onMessage: async (ws, event) => {
        try {
          let data = JSON.parse(await event.data.text());
          log("message received", data);

          if (data.hasOwnProperty("error")) {
            addError(data.error);
          } else if (data?.id === "list") {
            serviceNameMap.value = data.result;
          } else if (data?.id === "info") {
            let update_list = false;
            for (let id in data.result) {
              if (!serviceNameMap.value?.hasOwnProperty(id)) {
                update_list = true;
              }
              let info = data.result[id];
              if (info.start_time) {
                info.start_time = DateTime.fromISO(info.start_time);
              }

              if (info.end_time) {
                info.end_time = DateTime.fromISO(info.end_time);
              }
              if (servicesInfo.value === null) {
                servicesInfo.value = {};
              }
              servicesInfo.value[id] = info;
            }
            // info.value = data.result;
            if (update_list) {
              send('{"id":"list","method":"list"}');
            }
          } else if (data?.id === "stats") {
            for (let id in data.result) {
              let stats = data.result[id];
              if (!isNil(stats.uptime)) {
                stats.uptime = Duration.fromMillis(stats.uptime / 1e3);
              }
              if (!isNil(stats.cpu_time)) {
                stats.cpu_time = Duration.fromMillis(stats.cpu_time / 1e3);
              }
              servicesStats.value[id] = stats;
            }
          }
          await this.onMessage?.(data);
        } catch (e) {
          log("error", e);
        }
      },
    });
    this.ws = { ws, close, send };
    this.status = status;
    this.onMessage = null;
  }

  /**
   *
   * @param {string|object} request
   */
  send(request) {
    let data = typeof request === "string" ? request : JSON.stringify(request);
    log("sending", data);
    this.ws?.send?.(data);
  }

  close() {
    log("closing client");
    this.ws?.close?.();
  }

  /**
   *
   * @param {null|string} service
   */
  request_info(service) {
    this.send({ id: "info", method: "info", params: { service } });
  }

  /**
   *
   * @param {null|string} service
   */
  request_stats(service) {
    this.send({ id: "stats", method: "stats", params: { service } });
  }
}

export const client = new Client(api_url);
