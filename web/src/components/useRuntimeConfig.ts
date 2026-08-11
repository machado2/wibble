import {
  defaultPublicRuntimeConfig,
  type PublicRuntimeConfig,
} from "@/core/runtimeConfig";
import { useEffect, useState } from "react";

export const useRuntimeConfig = () => {
  const [config, setConfig] = useState<PublicRuntimeConfig>(
    defaultPublicRuntimeConfig
  );

  useEffect(() => {
    let cancelled = false;
    const load = () =>
      void fetch("/api/runtime-config")
        .then((response) => {
          if (!response.ok) throw new Error("Failed to load runtime config");
          return response.json() as Promise<PublicRuntimeConfig>;
        })
        .then((next) => {
          if (!cancelled) setConfig(next);
        })
        .catch((error) => console.error(error));
    load();
    const interval = window.setInterval(load, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  return config;
};
