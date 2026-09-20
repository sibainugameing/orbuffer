# Tauri Command Contracts

The desktop frontend communicates with Rust through Tauri commands. The TypeScript mapping for command arguments and results lives in `frontend/contracts.ts`.

## `aria2_start`

Starts or connects to the local aria2 RPC service.

Arguments:

| Name | Type | Optional | Default |
| --- | --- | --- | --- |
| `port` | number | yes | `6800` |
| `directory` | string | yes | aria2 default |
| `maxConcurrentDownloads` | number | yes | `3` |
| `split` | number | yes | `4` |
| `maxConnectionPerServer` | number | yes | `4` |
| `minSplitSize` | string | yes | `20M` |

Returns:

- `true` when OrBuffer started and owns the aria2 process.
- `false` when an existing aria2 process is already serving the endpoint.

## `aria2_add`

Adds a supported URI to aria2.

Arguments:

| Name | Type | Optional |
| --- | --- | --- |
| `uri` | string | no |
| `directory` | string or null | yes |
| `output` | string or null | yes |

Returns the aria2 GID as a string.

## `aria2_queue`

Returns an array containing active, waiting, and stopped aria2 downloads.

Each queue entry may contain fields such as:

- `gid`
- `status`
- `totalLength`
- `completedLength`
- `downloadSpeed`
- `uploadSpeed`
- `connections`
- `errorCode`
- `errorMessage`
- `files`

## `aria2_status`

Arguments:

| Name | Type |
| --- | --- |
| `gid` | string |

Returns the detailed aria2 status object for the requested GID.

## `aria2_active`

Returns the active aria2 downloads.

## `aria2_pause`

Arguments:

| Name | Type |
| --- | --- |
| `gid` | string |

Returns the affected GID.

## `aria2_resume`

Arguments:

| Name | Type |
| --- | --- |
| `gid` | string |

Returns the affected GID.

## `aria2_remove`

Arguments:

| Name | Type |
| --- | --- |
| `gid` | string |

Returns the affected GID.

## `aria2_clear_finished`

Removes stopped results whose aria2 status is `complete`, `error`, or `removed`.

Returns the number of removed results.

## `aria2_global`

Returns aria2 global transfer statistics, including `downloadSpeed`.

## Error contract

Tauri commands return `Result<T, String>`.

The frontend must treat the error string as user-visible diagnostic text and must not assume a successful value when the invocation rejects.

## Validation contract

`aria2_add` accepts only these URI schemes:

- HTTP
- HTTPS
- FTP
- FTPS
- SFTP
- magnet

The installed aria2 build remains responsible for the actual protocol capability.