export type Aria2File = {
  path?: string;
  length?: string;
  completedLength?: string;
  selected?: string;
};

export type DownloadVerification = "verified" | "mismatch" | "unavailable";

export type Download = {
  gid: string;
  status: "active" | "waiting" | "paused" | "error" | "complete" | "removed" | string;
  totalLength: string;
  completedLength: string;
  downloadSpeed: string;
  uploadSpeed: string;
  connections: string;
  errorCode?: string;
  errorMessage?: string;
  verification?: DownloadVerification;
  files?: Aria2File[];
};

export type Aria2StartArgs = {
  port?: number;
  directory?: string | null;
  maxConcurrentDownloads?: number;
  split?: number;
  maxConnectionPerServer?: number;
  minSplitSize?: string;
};

export type Aria2AddArgs = {
  uri: string;
  directory?: string | null;
  output?: string | null;
};

export type GidArgs = {
  gid: string;
};

export type Aria2GlobalStats = {
  downloadSpeed: string;
};

export type Aria2CommandMap = {
  aria2_start: {
    args: Aria2StartArgs;
    result: boolean;
  };
  aria2_add: {
    args: Aria2AddArgs;
    result: string;
  };
  aria2_active: {
    args: undefined;
    result: Download[];
  };
  aria2_queue: {
    args: undefined;
    result: Download[];
  };
  aria2_status: {
    args: GidArgs;
    result: Download;
  };
  aria2_pause: {
    args: GidArgs;
    result: string;
  };
  aria2_resume: {
    args: GidArgs;
    result: string;
  };
  aria2_remove: {
    args: GidArgs;
    result: string;
  };
  aria2_clear_finished: {
    args: undefined;
    result: number;
  };
  aria2_global: {
    args: undefined;
    result: Aria2GlobalStats;
  };
};
