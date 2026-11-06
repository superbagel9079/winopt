# NVIDIA driver profiles through NVIDIA Profile Inspector: research

Status: research only, no app code. Researched 2026-11-06. Latest GeForce driver seen during research: 621.14 WHQL (TechPowerUp driver list). NVIDIA Profile Inspector (NPI) latest stable: v3.0.2.1 (2026-07-05); latest pre-release: v3.0.2.3 (2026-10-13).

How to read this document:

- **[V]** means verified in a primary source (NVIDIA header or docs, NPI source code, a measured review) and cited.
- **[S]** means the claim comes from a secondary source (news site, summary of a video) that I read but whose underlying data I could not inspect.
- **[U]** means unverified: forum reports, inference by me, or something I could not confirm. Do not build hard logic on [U] items without testing.
- Setting IDs and value IDs come from two places, and both were read directly: NVIDIA's open NVAPI header `NvApiDriverSettings.h` (source 8) and NPI's `CustomSettingNames.xml` (source 2). Where only NPI knows the ID (the setting is undocumented by NVIDIA), this is marked "NPI only".
- In a `.nip` file, setting IDs and DWORD values are written in **decimal**, not hex (see section 3).

---

## 1. Settings that matter

General finding first: most driver profile settings have **no measurable FPS effect** on modern GPUs. The settings with clear, measured effects on latency are the ones that control the **render queue** (Low Latency Mode, frame caps) and **sync** (V-SYNC, G-SYNC, frame cap below refresh). Everything else is mostly image quality, compatibility, or folklore.

Columns: NVCP name / NPI name, ID (hex), values (hex), what it does, measured effect, sources.

### 1.1 Latency and sync

| NVCP name / NPI name | Setting ID | Values | What it does | Measured effect (FPS, frametime, latency) | Sources |
|---|---|---|---|---|---|
| **Low Latency Mode** (NVCP) / NPI: "Maximum Pre-Rendered Frames" + "Ultra Low Latency - Enabled" + "Ultra Low Latency - CPL State" | MPRF: `0x007BA09E` [V, NVAPI `PRERENDERLIMIT_ID`]. ULL Enabled: `0x10835000` [V, NPI only]. ULL CPL State: `0x0005F543` [V, NPI only] | MPRF: `0x00` = use 3D app setting, `0x01` to `0x08` = 1 to 8 frames. ULL Enabled: `0x0` Off, `0x1` On. CPL State: `0x0` Off, `0x1` On, `0x2` Ultra (NPI says this only records the NVCP UI state). | Limits how many frames the CPU may queue ahead of the GPU. NVIDIA: Off lets the engine queue 1 to 3 frames; **On is the same as Max Pre-Rendered Frames = 1**; Ultra submits the frame "just in time" for the GPU. | NVIDIA claims Ultra reduces latency "by up to 33%", most effective when **GPU-bound at 60 to 100 FPS** [V vendor claim, no workload stated]. Blur Busters: MPRF 1 "at best" saves 1 frame, at worst causes more frametime spikes if the CPU cannot keep up; many games already use 1 internally; effects are system and game dependent [V]. Battle(non)sense found a frame cap beats driver Low Latency Mode for latency [S]. In CPU-bound situations the queue is already empty, so expect no gain [U, follows from the mechanism]. | 11, 14, 21 |
| (same) API coverage | | | Originally DX9 and DX11 only. DX12 support added in driver **551.23** (Jan 2024). **Not supported in Vulkan** (Blur Busters, current). | | 11, 14, 17 |
| (same) Ultra with G-SYNC | | | With G-SYNC + NVCP V-SYNC, Ultra also auto-caps FPS below refresh: about 59 @60 Hz, 97 @100, 116 @120, 138 @144, 224 @240, in supported games. | Auto-cap verified as Blur Busters guidance [V]; per-game reliability reported as inconsistent (for example not triggered in a Vulkan game) [U, forum]. | 14, 16 |
| **NVIDIA Reflex** (in-game, not a driver setting) | n/a | n/a | Engine-side just-in-time frame submission. | NVIDIA: if both Reflex and driver Low Latency Mode are enabled, **Reflex takes priority**. Blur Busters: Reflex "will override" LLM regardless of NVCP value. Reflex is supported on GTX 900 series and newer. Reflex "Boost" keeps clocks up, similar to "Prefer maximum performance". | 12, 14 |
| **Max Frame Rate** (NVCP) / NPI: "Frame Rate Limiter V3" | `0x10835002` [V, NVAPI `FRL_FPS_ID`] | `0x0` = Off, otherwise the FPS value, max `0x3FF` (1023) | Driver-side FPS cap, added in driver **441.87**. | Blur Busters: Max Frame Rate is a CPU-level limiter "comparable to the RTSS framerate limiter in both frametime performance and added delay" [V]. RTSS adds "up to 1 frame" of delay vs an in-game limiter, measured in Overwatch [V]. Older NVIDIA limiters (v1, v2, set via NPI) added 2 to 3.5 frames with G-SYNC; that result does **not** apply to V3 [V]. Battle(non)sense: V3 latency roughly matches RTSS at 60 and 144 FPS, with very stable frametimes [S]. **Ranking for latency: in-game limiter (or Reflex) first, then driver Max Frame Rate approximately equal to RTSS.** Capping below the GPU limit is the most reliable latency reduction when not using Reflex [S, Battle(non)sense via Tom's Hardware]. | 14, 15, 21, 22 |
| NPI: "Frame Rate Limiter - Background Application" (NVCP: "Background Application Max Frame Rate") | `0x10835005` [V, NPI only] | `0x0` Off, otherwise FPS | Caps FPS when the game is not in focus. | Power and heat only, no foreground effect [U]. | 2 |
| **Vertical sync** | `0x00A879CF` [V, NVAPI `VSYNCMODE_ID`] | `0x60925292` use 3D app setting (default), `0x08416747` Force off, `0x47814940` Force on, `0x18888888` Fast Sync (`VSYNCMODE_VIRTUAL`), `0x32610244` 1/2 refresh, `0x71271021` 1/3, `0x13245256` 1/4 | Driver V-SYNC override. | Blur Busters: with G-SYNC, use **NVCP V-SYNC On** plus a cap at least 3 FPS below refresh; then V-SYNC never engages, and NVCP V-SYNC has no lag penalty over in-game V-SYNC but avoids in-game triple-buffer or pacing quirks [V]. Without VRR, V-SYNC On adds latency (classic result; Blur Busters measured V-SYNC delay stacking with RTSS delay) [V]. | 8, 14, 15 |
| NPI: "Vertical Sync - Tear Control" (NVCP: "Adaptive" V-SYNC) | `0x005A375C` [V] | `0x96861077` Standard (default), `0x99941284` Adaptive | Turns V-SYNC off when FPS falls below refresh. | No measurement found [U]. Leave default. | 8 |
| **Monitor Technology / G-SYNC** (per-app) / NPI: "GSYNC - Application State" | `0x10A879CF` [V, NVAPI `VRR_APP_OVERRIDE_ID`] | `0` Allow, `1` Force off, `2` Disallow, `3` ULMB, `4` Fixed refresh | Per-app G-SYNC permission. | Enables VRR behaviour above; no separate measurement. | 2, 8 |
| G-SYNC global mode / NPI: "GSYNC - Application Mode", "GSYNC - Global Mode", "GSYNC - Global Feature" | `0x1194F158` (NVAPI `VRR_MODE_ID`, NVAPI calls it "Enable G-SYNC globally"; NPI labels it "Application Mode"), `0x1094F1F7` (`VRRREQUESTSTATE_ID`), `0x1094F157` (`VRRFEATUREINDICATOR_ID`) [V] | Mode: `0` Off, `1` Fullscreen only, `2` Fullscreen and windowed. Feature: `0`/`1`. | G-SYNC enablement is normally done in "Set up G-SYNC", which is a display setting. The naming of these IDs differs between NVIDIA and NPI. | **Recommendation: winopt should not write these.** Leave G-SYNC enablement to NVCP / NVIDIA App. [U: I could not confirm whether DRS values alone fully enable G-SYNC.] | 2, 8 |
| NPI: "Vertical Sync - Smooth AFR Behavior" | `0x101AE763` | `0`/`1` | SLI only. | Irrelevant on current single-GPU systems. | 8 |
| **Triple buffering** | `0x20FDD1F9` [V, NVAPI `OGL_TRIPLE_BUFFER_ID`] | `0x0` Off, `0x1` On | **OpenGL only** (the NVAPI name is `OGL_`). Only matters with V-SYNC On without VRR. | No measurement found. No effect on DX or Vulkan games [V by naming; behaviour U]. | 8 |
| **Preferred refresh rate** / NPI: "Prefered Refreshrate" (sic) | `0x0064B541` [V, `REFRESH_RATE_OVERRIDE_ID`] | `0x0` Application controlled, `0x1` Highest available | Makes exclusive-fullscreen apps that request a default (often 60 Hz) mode use the highest refresh instead. | No benchmark; effect is binary (wrong refresh vs right refresh) and obvious when it applies, mostly old DX9/DX11 exclusive-fullscreen titles [U]. | 2, 8 |
| NPI: "Vulkan/OpenGL Present Method" (NVCP: same name) | `0x20D690F8` [V] | `0x0` Prefer native, `0x1` Prefer layered on DXGI swapchain, `0x2` Auto | Lets Vulkan/OpenGL present through DXGI (flip model, Windows features). | No measurement found [U]. Leave Auto. | 2, 8 |

### 1.2 Power, cache, threading

| NVCP name / NPI name | Setting ID | Values | What it does | Measured effect | Sources |
|---|---|---|---|---|---|
| **Power management mode** / NPI: "Power Management - Mode" | `0x1057EB71` [V, `PREFERRED_PSTATE_ID`] | `0x0` Adaptive, `0x1` Prefer maximum performance, `0x5` Optimal power (driver default, shown as "Normal" in current NVCP / NVIDIA App). NVAPI also defines `0x2` driver controlled, `0x3` prefer consistent performance, `0x4` prefer min, which NPI does not expose. | Controls how eagerly the GPU drops clocks. | TheFPSReview (RTX 2060 SUPER, RTX 2080 SUPER, DX11/DX12/Vulkan): **no consistent FPS difference** between modes; differences random and within noise; Prefer maximum performance made **idle power "skyrocket"** and on the 2080 SUPER lowered clocks slightly (1920 to 1905 MHz) [V]. Latency: NVIDIA says Prefer maximum performance makes a similar power-for-latency trade as Reflex Boost, which helps mainly when **CPU-bound or frame-capped** (GPU underused, clocks drop, render time grows) [V vendor statement, size not quantified]. | 12, 19 |
| **Shader Cache Size** / NPI: "Shader Cache - Cache Size" | `0x00AC8497` [V, `PS_SHADERDISKCACHE_MAX_SIZE_ID`] | Value in MB per NPI labels: `0x0` Off (disable), `0x80` 128 MB, `0x100` 256 MB, `0x200` 512 MB, `0x400` 1 GB, `0x1000` 4 GB, `0x1400` 5 GB, `0x2000` 8 GB, `0x2800` 10 GB, `0x4000` 16 GB, `0x19000` 100 GB, `0xFFFFFFFF` Unlimited. NVAPI `DEFAULT = 0x4000`. | Max disk space for compiled shaders. Added in driver 496.13. | **No measured study found.** Mechanism: a too-small cache evicts shaders, forcing recompiles that show up as stutter (frametime spikes), especially with large DX12 libraries [U]. Setting it to Off clearly causes recompilation every launch [U, logical]. What NVCP "Driver Default" maps to in MB is unverified. | 2, 8, 18 |
| NPI: "Shader Cache - Enabled" | `0x00198FFF` [V, `PS_SHADERDISKCACHE_ID`] | `0x0` Off, `0x1` On (default) | Master switch for DX shader disk cache. | Keep On. | 8 |
| NPI: "Shader Cache - Pre-compile Shader Options" (NVIDIA App: Auto Shader Compilation, beta) | `0x00EAD189` [V, NPI only, min driver 595.97] | `0x0` Disabled, `0x1` Low, `0x2` Medium, `0x3` High | Recompiles DX12 shaders while the PC is idle after driver updates. | Press coverage only; no independent measurement [S]. Uses CPU in the background. | 2, 29 |
| **Threaded optimization** | `0x20C1221E` [V, `OGL_THREAD_CONTROL_ID`] | `0x0` Auto (default), `0x1` On, `0x2` Off | Driver worker threads. NVIDIA names it `OGL_`, so it certainly applies to OpenGL; whether it affects DX11 is commonly claimed but **unverified**. | **Evidence mixed, forum only**: one 5950X user got 324 vs 306 avg FPS with it Off, others saw zero difference, one laptop lost FPS with it Off [U]. No controlled review found. Leave Auto. | 8, 33, 34 |
| NPI: "CUDA - Force P2 State" | `0x50166C5E` [NPI only] | | Compute memory clock state. | Not gaming relevant. Do not touch. | 2 |

### 1.3 Texture filtering and antialiasing (image quality)

| NVCP name / NPI name | Setting ID | Values | What it does | Measured effect | Sources |
|---|---|---|---|---|---|
| **Texture filtering - Quality** | `0x00CE2691` [V, `QUALITY_ENHANCEMENTS_ID`] | `0xFFFFFFF6` High quality, `0x00000000` Quality (default), `0x0000000A` Performance, `0x00000014` High performance | Preset that toggles the trilinear and anisotropic sample optimizations below. | **No modern controlled benchmark found.** Old data: GeForce 7 (G70) lost performance with High quality, G80 (2006) "takes it in stride" [S, AnandTech]. One flight-sim user measured 54.2 vs 54.4 FPS for Quality vs High quality [U]. Conclusion: expect near zero FPS difference on Pascal and newer; latency effect none [U]. | 8, 2 |
| NPI: "Texture Filtering - Quality Substitution" | `0x00CE2692` [V] | `0x0` No substitution, `0x1` High quality becomes quality | Driver may downgrade High quality to Quality. | No measurement. Leave default. | 8 |
| **Texture filtering - Trilinear optimization** | `0x002ECAF2` [V, `PS_TEXFILTER_DISABLE_TRILIN_SLOPE_ID`] | NPI labels: `0x0` On (ignored under High quality), `0x1` Off | Brilinear filtering shortcut. | No modern measurement [U]. | 2, 8 |
| **Texture filtering - Anisotropic sample optimization** / NPI: "Anisotropic Filter - Sample Optimization" | `0x00E73211` [V, `PS_TEXFILTER_ANISO_OPTS2_ID`] | `0x0` Off, `0x1` On | Fewer AF samples. | No modern measurement [U]. | 2, 8 |
| NPI: "Anisotropic Filter - Optimization" | `0x0084CD70` [V, `PS_TEXFILTER_BILINEAR_IN_ANISO_ID`] | `0x0` Off, `0x1` On | Bilinear instead of trilinear inside AF. | No modern measurement [U]. | 2, 8 |
| **Texture filtering - Negative LOD bias** | `0x0019BB68` [V, `PS_TEXFILTER_NO_NEG_LODBIAS_ID`] | `0x0` Allow (default), `0x1` Clamp | Clamps negative mip bias. | No FPS effect expected. Clamping can blur textures in games that rely on negative bias (DLSS and other temporal upscalers set one) [U, inference]. Leave Allow. | 2, 8 |
| **Anisotropic filtering** | Mode `0x10D2BB16` (`ANISO_MODE_SELECTOR_ID`), Level `0x101E61A9` (`ANISO_MODE_LEVEL_ID`) [V] | Mode: `0x0` application-controlled, `0x1` user-defined. Level: `0x1` off (linear), `0x2` 2x, `0x4` 4x, `0x8` 8x, `0x10` 16x. (NPI lists "6x" with the same value `0x4` as 4x; looks like an NPI data bug [U].) | Forces AF level. | Historical: 1x to 16x cost about 8% on GeForce 7800 GTX in 3DMark03 [S, Beyond3D]. **No modern measurement found**; widely believed to be 0 to 2% on current GPUs [U]. Whether driver-forced AF applies in DX12/Vulkan is unverified [U]. | 2, 8 |
| NPI: "Prevent Anisotropic Filtering" | `0x103BCCB5` [NPI] | `0`/`1` | Compatibility flag NVIDIA sets on some game profiles. | Do not touch. | 2 |
| **Antialiasing - Mode / Setting** (MSAA override) | Mode `0x107EFC5B` (`AA_MODE_SELECTOR_ID`), Method `0x10D773D2` (`AA_MODE_METHOD_ID`) [V] | Mode: `0x0` app-controlled, `0x1` override, `0x2` enhance. Method: `0x0` none, `0x10` 4x MSAA, `0x25` 8x MSAA, and about 50 more. | Forces MSAA. | Generally only works in DX9 and some DX11/OpenGL titles; modern deferred and temporal renderers ignore it [U]. Costs FPS where it applies. | 8 |
| **Antialiasing - Transparency** | Multisampling `0x10FC2D9C`, Supersampling `0x10D48A85` [V] | see NVAPI enums | Alpha-test AA. | Same compatibility limits [U]. | 8 |
| **Antialiasing - Gamma correction** | `0x107D639D` [V] | `0x0` off, `0x1` on if FOS, `0x2` always | | No effect without MSAA. | 8 |
| **Antialiasing - FXAA** | `0x1074C972` [V, `FXAA_ENABLE_ID`] | `0x0` Off (default), `0x1` On | Driver post-process FXAA. | Small FPS cost, blurs UI text [U]. Off. | 8 |
| **Antialiasing - MFAA** | `0x0098C1AC` [V, `MAXWELL_B_SAMPLE_INTERLEAVE_ID`] | `0x0` Off, `0x1` On | Only acts with MSAA. | No effect without MSAA [U]. | 8 |
| **Image Sharpening** / NPI: "Sharpening Filter - Enabled", "Sharpening - Value" | `0x00598928`, `0x002ED8CD` [NPI only] | | Driver sharpening. | Not measured; outside scope of presets. | 2 |

### 1.4 Resizable BAR (rBAR)

| NPI name | Setting ID | Values | What it does | Measured effect and risks | Sources |
|---|---|---|---|---|---|
| "rBAR - Enable" | `0x000F00BA` [NPI only, also documented by TechPowerUp] | `0x0` Disabled, `0x1` Enabled | Per-profile rBAR enable. NVIDIA enables rBAR only for whitelisted games after testing. | TechPowerUp (RTX 30 launch, 22 games): **2 to 4% average** gain in whitelisted titles, about 20% in Gears 5, **regression at 1080p** in Watch Dogs Legion (FPS limited to about 80) even though it was whitelisted [V]. Forcing rBAR on non-whitelisted games: some gain (Hogwarts Legacy, a 10% 3DMark Port Royal claim) and some "devastating" losses [S, Digital Trends / Tom's Hardware]. | 20 |
| "rBAR - Options" | `0x000F00BB` [NPI only] | bitfield | | | 2, 20 |
| "rBAR - Size Limit" | `0x000F00FF` [NPI only] | QWORD bytes, e.g. `0x40000000` = 1 GB (typical whitelisted value) | | | 2, 20 |
| "rBAR - Enable v2" | `0x000BFA21` [NPI only, min driver 616.56 per NPI] | `0x0` Disabled, `0x1` Auto, `0x2` Enabled | Newer master switch. | **Unverified**: I found no NVIDIA note describing it. | 2 |
| "rBAR - Intel CPU Exclusion", "rBAR - Flags" | `0x00E942FC`, `0x000F00BE`, `0x000F00BF` [NPI only] | | | Unknown semantics. | 2 |

Risk summary: rBAR also needs firmware support (Above 4G decoding, rBAR on in BIOS, CSM off, VBIOS support). Forcing it in the **Base Profile** applies it to every game, including games where it regresses. **winopt should not force rBAR in any preset.** At most offer it as a per-game experiment with a before/after benchmark.

### 1.5 DLSS overrides

| NPI name (NVIDIA App name) | Setting ID | Values | What it does | Evidence | Sources |
|---|---|---|---|---|---|
| "DLSS - Enable DLL Override" (NVAPI `NGX_DLSS_SR_OVERRIDE_ID`) | `0x10E41E01` [V] | `0x0` Off, `0x1` On | Use the driver's newest DLSS-SR model instead of the game's DLL. DLSS 2+ games only; some games block it. | | 2, 8 |
| "DLSS - Forced Preset Letter" (NVIDIA App: "DLSS Override - Model Presets"; NVAPI `NGX_DLSS_SR_OVERRIDE_RENDER_PRESET_SELECTION_ID`) | `0x10E41DF3` [V] | `0x0` N/A, `0x00FFFFFF` use recommended, `0x1` to `0x6` presets A to F (CNN), `0xA` J, `0xB` K (transformer gen 1), `0xC` L, `0xD` M (transformer gen 2), `0xE` N, `0xF` O [NPI labels] | Forces a DLSS model. | NVIDIA (Jan 2026, DLSS 4.5): "Recommended" = Preset M for Performance, L for Ultra Performance, K for other modes. **RTX 20 and 30 lack native FP8, so models M and L "carry a heavier performance impact"** there; NVIDIA suggests staying on K [V]. | 2, 8, 23 |
| "DLSS - Forced Model Preset Profile" | `0x00634291` [NPI only] | `0x0` N/A, `0x1` Recommended, `0x2` Custom | NPI says some games need this set for the preset letter to apply. | [U] | 2 |
| "DLSS - Forced Quality Level" (`NGX_DLSS_SR_MODE_ID`) | `0x10AFB768` [V] | `0` Performance, `1` Balanced, `2` Quality, `3` snippet controlled (default), `4` DLAA, `5` Ultra Performance, `6` Custom | Forces a quality level. | Changes FPS strongly but overrides the game's own choice; not suitable for a global preset. | 8 |
| "DLSS - Override DLSS modes to be DLAA" | `0x10E41DF4` [V] | `0` default, `1` DLAA | | Costs FPS; quality only. | 8 |
| "DLSS-RR / DLSS-FG - Enable DLL Override", "Forced Preset Letter" | `0x10E41E02`, `0x10E41E03`, `0x10E41DF7`, `0x10E41DF1` [V] | | Ray Reconstruction and Frame Generation equivalents. | Frame generation **adds latency** relative to the same FPS without it (Blur Busters author, explanation not a measurement) [S]. Do not force FG in a latency-focused preset. | 8, 15 |
| "DLSS - Enable Streamline Override" | `0x10E41E06` [V, NPI min driver 616.56] | `0`/`1` | | [U] | 2, 8 |

Known friction [U, forum]: the NVIDIA App has been reported to reset the global DLSS override to 0 for games it does not list as supported. If winopt writes DLSS overrides, it should re-check them after the NVIDIA App runs.

### 1.6 Laptop and other

| Name | Setting ID | Values | Notes | Sources |
|---|---|---|---|---|
| Battery Boost max FPS (`BATTERY_BOOST_APP_FPS_ID`) | `0x10115C8C` [V] | `0x0` no override, `0x1` to `0x3FF` FPS | Laptop on battery FPS cap. Leave to the user / NVIDIA App. | 8 |
| Optimus: "Enable application for Optimus" (`SHIM_RENDERING_MODE_ID`) | `0x10F9DC81` [V] | `0x0` integrated, `0x1` enable (dGPU), `0x10` auto select (default) | Per-app GPU selection on hybrid laptops. **Never set in the Base Profile**: it would force every process onto one GPU. | 8 |
| Smooth Motion (driver frame generation) | `0xB0D384C0` [NPI only] | | Driver-level frame generation; adds latency like any frame generation [U]. Keep off in presets. | 2 |

---

## 2. Presets

Design principle: only change what has evidence, leave everything else at driver default ("use the 3D application setting"). All presets are written into the **Base Profile** in **merge** mode (section 3), so game-specific NVIDIA profiles keep their compatibility flags.

Machine facts winopt must detect first:

- GPU generation: Pascal (GTX 10), Turing (GTX 16 / RTX 20), Ampere (RTX 30), Ada (RTX 40), Blackwell (RTX 50).
- Desktop vs laptop; on laptops: Optimus active, MUX in dGPU mode, Advanced Optimus.
- VRR active on the primary display (G-SYNC or G-SYNC Compatible) and its max refresh rate.
- VRAM size, free disk space on the shader cache drive.
- Driver version (gate settings: LLM DX12 needs 551.23+, Max Frame Rate needs 441.87+, Auto Shader Compilation 595.97+).

Let `R` = display max refresh rate. VRR cap: Blur Busters says at least 3 FPS below `R`. Reflex and LLM Ultra auto-caps follow approximately `R - R*R/3600` (144 to 138, 240 to 224, 120 to 116, 60 to 59); that formula is my fit to Blur Busters' published numbers [U], so use `min(R - 3, round(R - R*R/3600))` as a safe cap.

### 2.1 Table

| Setting | Performance (lowest latency) | Optimized (default pick) | Quality | Reason |
|---|---|---|---|---|
| Low Latency Mode | **Ultra** (ULL `0x10835000`=1, CPL `0x0005F543`=2, MPRF `0x007BA09E`=1) | **On** (MPRF=1, ULL=0, CPL=1) | **Off** (MPRF=0 app, ULL=0, CPL=0) | NVIDIA: Ultra up to 33% lower latency when GPU-bound; On = MPRF 1; Blur Busters: can add frametime spikes on weak CPUs, so Optimized uses On and Quality leaves the engine's queue for throughput. Reflex supersedes all three in Reflex games. |
| Max Frame Rate (`0x10835002`) | VRR: cap formula above. No VRR: **Off** (assume in-game cap / Reflex) | VRR: cap formula. No VRR: Off | VRR: cap formula. No VRR: Off | Blur Busters: cap below refresh keeps G-SYNC out of V-SYNC lag. Driver cap ~ RTSS, worse than in-game, so not forced on non-VRR. |
| Vertical sync (`0x00A879CF`) | VRR: **Force on** (`0x47814940`). No VRR: **app setting** | VRR: Force on. No VRR: app setting | VRR: Force on. No VRR: app setting | Blur Busters G-SYNC 101 optimal settings. Forcing V-SYNC off globally is avoided because tearing preference is the user's call. |
| G-SYNC app state (`0x10A879CF`) | leave default (Allow) | leave | leave | Enablement belongs to the display setup. |
| Power management (`0x1057EB71`) | Desktop: **Prefer maximum performance** (`0x1`). Laptop: Normal (`0x5`) | **Normal / Optimal power** (`0x5`) | Normal (`0x5`) | TheFPSReview: no FPS gain, much higher idle power. NVIDIA: latency benefit only when CPU-bound or capped. Only worth it for latency-first desktops. |
| Shader cache size (`0x00AC8497`) | **Unlimited** (`0xFFFFFFFF`) if free space > 100 GB, else 10 GB (`0x2800`) | same | same | Avoid eviction recompiles; no measured study [U]. Never "Off". |
| Threaded optimization (`0x20C1221E`) | Auto (no write) | Auto | Auto | Evidence mixed, forum only. |
| Texture filtering quality (`0x00CE2691`) | Quality (no write) | Quality (no write) | **High quality** (`0xFFFFFFF6`) | No measured FPS gain from Performance modes on modern GPUs; High quality costs ~nothing [U] and removes optimizations. |
| Trilinear opt., AF sample opt., AF optimization | no write | no write | no write (High quality disables them) | Follow the quality preset. |
| Negative LOD bias (`0x0019BB68`) | Allow (no write) | Allow | Allow | Clamp can blur temporally upscaled games [U]. |
| Anisotropic filtering | app-controlled | app-controlled | **16x** (mode `0x1`, level `0x10`) | Historical cost small; modern cost unmeasured [U]; forcing may not apply in DX12/Vulkan [U]. |
| AA overrides, FXAA, MFAA, transparency AA | app-controlled / Off | same | same | Driver AA rarely applies to modern renderers; FXAA blurs UI. |
| Triple buffering (`0x20FDD1F9`) | no write (Off) | no write | no write | OpenGL only; with VRR not wanted. |
| Preferred refresh rate (`0x0064B541`) | **Highest available** (`0x1`) | Highest available | Highest available | Prevents old exclusive-fullscreen games running at 60 Hz on high-refresh panels. Skip on laptops when the user runs a reduced refresh on battery [U]. |
| rBAR flags | no write | no write | no write | Whitelist exists for a reason; regressions measured. |
| DLSS override (`0x10E41E01`, `0x10E41DF3`) | Pascal / GTX 16: n/a. RTX 20/30: **Override on, preset K** (`0xB`). RTX 40/50: no write (game default) | RTX 20/30: Override on, preset K. RTX 40/50: Override on, **Recommended** (`0x00FFFFFF`) | RTX 20/30: Override on, Recommended (accept the cost). RTX 40/50: Override on, Recommended | NVIDIA: M/L heavier on RTX 20/30 (no FP8). Performance preset avoids heavier models where it can. |
| DLSS FG / Smooth Motion | no write | no write | no write | Frame generation adds latency. |
| Background app FPS (`0x10835005`) | no write | Laptop only: 30 | no write | Power only [U]. |
| Pre-compile shaders (`0x00EAD189`) | no write | no write | no write | Leave to NVIDIA App opt-in. |

### 2.2 What must depend on the machine

- **VRR display (G-SYNC / G-SYNC Compatible)**: changes V-SYNC and Max Frame Rate in every preset (section 2.1). On a 60 Hz VRR panel the cap is 57 to 59 FPS, which may feel limiting; still correct for latency per Blur Busters.
- **Refresh rate**: drives the cap value. Without VRR, Low Latency Mode Ultra cannot auto-cap, and an uncapped GPU-bound game builds a queue; in that case winopt should recommend an in-game cap or Reflex rather than writing a driver cap blindly.
- **GPU generation**:
  - Pascal (GTX 10): no DLSS, no Reflex Boost differences; Low Latency Mode and Max Frame Rate supported (LLM "support for all GPUs" per NVIDIA). G-SYNC Compatible on GTX 10 series and newer [U: not re-verified here].
  - Turing / Ampere: DLSS preset K rather than M/L for performance (NVIDIA FP8 note).
  - Ada / Blackwell: Recommended DLSS presets cheap enough; Blackwell is where Multi Frame Generation lives, keep it off in presets.
- **Laptop vs desktop**: never use Prefer maximum performance on laptops (idle power and heat measured as much higher on desktop cards; on laptops this costs battery and thermal headroom [U on laptops]). Do not touch Optimus shim IDs in the Base Profile.
- **Optimus / MUX**: with Optimus active, frames go through the iGPU, which adds latency and usually blocks G-SYNC on the internal panel; a MUX or Advanced Optimus in dGPU mode removes that path [S, vendor explainers; one estimate is about 2 ms latency and 5 to 15% FPS, unverified]. winopt should detect the display path and only apply VRR logic if the NVIDIA GPU drives the panel. Switching the MUX is a firmware/OEM action, outside NPI.
- **VRAM size**: no driver profile setting found with a measured VRAM-dependent effect. Indirect: DLSS model and texture settings are in-game. rBAR is unrelated to VRAM size in a way winopt can act on. [U]
- **CPU-bound vs GPU-bound**: Low Latency Mode and frame caps help mostly when **GPU-bound** (NVIDIA, Battle(non)sense). When CPU-bound the render queue is already short; Ultra may cost FPS and Prefer maximum performance or Reflex Boost may help a little (NVIDIA). winopt can classify from PresentMon: GPU-bound if `MsGPUBusy` is close to the frame time, CPU-bound if `MsCPUBusy` dominates (section 4).
- **Reflex games**: when Reflex is on in-game, the driver Low Latency Mode is ignored (NVIDIA, Blur Busters). The Max Frame Rate cap and V-SYNC/G-SYNC settings still matter. DLSS Frame Generation forces Reflex on.

---

## 3. Applying it

### 3.1 Install NPI via Chocolatey

- Package id: **`nvidia-profile-inspector`** (verified exists, approved as trusted 2026-07-18, version 3.0.2.1). Maintainer: **jtcmedia** (community maintainer, not Orbmu2k; Chocolatey states it is not affiliated with the author). Package source: https://github.com/jtcmedia/chocolatey-packages/tree/master/nvidia-profile-inspector.
- Command: `choco install nvidia-profile-inspector -y --params "/NoDesktopIcon"`. Without `/NoDesktopIcon` the install script creates a desktop shortcut.
- The install script unzips the release into the package `tools` folder; the exe is reachable via the Chocolatey shim `%ChocolateyInstall%\bin\nvidiaProfileInspector.exe`.
- Chocolatey's VirusTotal note: at least one file has between 1 and 4 detections (common for NPI because it edits driver settings; still worth showing the user).
- Requirements (NPI README): Windows, NVIDIA GPU and driver, **.NET Framework 4.8**, admin rights.
- Alternative without Chocolatey: download the release zip from the official repo releases and verify it came from `github.com/Orbmu2k/nvidiaProfileInspector/releases`.

Official repository: https://github.com/Orbmu2k/nvidiaProfileInspector, **MIT license** (GitHub reports SPDX `MIT`). The app is now WPF on .NET Framework 4.8.

### 3.2 Command line (verified in `App.xaml.cs`, current master)

| Argument | Effect |
|---|---|
| `file.nip [file2.nip ...]` | Import; if a profile already exists and no mode switch is given, NPI shows a merge/replace prompt. |
| `-silentImport` or `-silent` | No success dialog. **If no mode is given, silent defaults to Replace.** |
| `-mergeImport` | Merge: keep existing values, overwrite only settings present in the file. |
| `-replaceImport` | Replace: reset each target profile, then write only what the file contains. |
| `-exportCustomized` | Export all customized profiles to `CustomProfiles_<yyyy-MM-dd_HH-mm-ss>.nip` **next to the exe**, then exit. |
| `-createCSN` | Write `CustomSettingNames.xml` to the current dir and exit. |
| `-showOnlyCSN`, `-disableScan` | UI options. |

Recommended winopt call: `nvidiaProfileInspector.exe -silentImport -mergeImport "C:\...\winopt-optimized.nip"`.

Important behaviours from the source:

- **Replace on the Base Profile resets it** (`ResetProfile`) and deletes any user setting not in the file. Silent import without `-mergeImport` therefore wipes the user's other global tweaks. Use merge for presets; use replace only for restoring a full backup.
- Merge mode cannot "unset" a value; to revert winopt's writes, restore the backup (replace) or write the default values explicitly.
- If an NPI window is already running, the import is forwarded to it via a window message; winopt should make sure no NPI instance is open, or accept that.
- Errors in silent mode: the success box is suppressed, but the code still shows an error message box on an exception, and a warning box only when not silent. winopt should verify the result by re-exporting rather than trusting the exit code (the code calls `Shutdown()` without a distinct exit code) [V by reading code; exit code behaviour U].
- Settings the driver does not know (`NVAPI_SETTING_NOT_FOUND`) are silently skipped.

### 3.3 Admin rights

Yes. `app.manifest` has `requestedExecutionLevel level="requireAdministrator"`, and the README lists administrator privileges. Launching it from a non-elevated process triggers UAC; winopt should run the import from its elevated helper.

### 3.4 `.nip` format

XML, serialized by .NET `XmlSerializer` from `List<Profile>`; NPI writes it as UTF-16 (`Encoding.Unicode`). Root `ArrayOfProfile`. `SettingID` is a **decimal uint**, `SettingValue` is a **decimal string** for `Dword`/`Qword`, text for `String`/`AnsiString`, base64 for `Binary`. `SettingNameInfo` is informational only. `ValueType` is one of `Dword`, `AnsiString`, `String`, `Binary`, `Qword`. The global profile is addressed by its name, `Base Profile` (an empty `ProfileName` maps to the current global profile in the code).

Minimal example: Optimized preset on a 144 Hz G-SYNC desktop (LLM On, cap 138, V-SYNC force on, Highest refresh, shader cache 10 GB):

```xml
<?xml version="1.0" encoding="utf-16"?>
<ArrayOfProfile>
  <Profile>
    <ProfileName>Base Profile</ProfileName>
    <Executeables />
    <Settings>
      <ProfileSetting>
        <SettingNameInfo>Maximum Pre-Rendered Frames</SettingNameInfo>
        <SettingID>8102046</SettingID>
        <SettingValue>1</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Ultra Low Latency - Enabled</SettingNameInfo>
        <SettingID>277041152</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Ultra Low Latency - CPL State</SettingNameInfo>
        <SettingID>390467</SettingID>
        <SettingValue>1</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Frame Rate Limiter V3</SettingNameInfo>
        <SettingID>277041154</SettingID>
        <SettingValue>138</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Vertical Sync</SettingNameInfo>
        <SettingID>11041231</SettingID>
        <SettingValue>1199655232</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Prefered Refreshrate</SettingNameInfo>
        <SettingID>6600001</SettingID>
        <SettingValue>1</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Shader Cache - Cache Size</SettingNameInfo>
        <SettingID>11306135</SettingID>
        <SettingValue>10240</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
    </Settings>
  </Profile>
</ArrayOfProfile>
```

Decimal conversions used: `0x007BA09E`=8102046, `0x10835000`=277041152, `0x0005F543`=390467, `0x10835002`=277041154, `0x00A879CF`=11041231, `0x47814940`=1199655232 (Force on), `0x0064B541`=6600001, `0x00AC8497`=11306135, `0x1057EB71`=274197361, `0x20C1221E`=549528094, `0x00CE2691`=13510289, `0x10A879CF`=279476687. Other V-SYNC values: `0x60925292`=1620202130 (app), `0x08416747`=138504007 (off), `0x18888888`=411601032 (Fast).

Caveat on the Low Latency Mode mapping: NVIDIA states On equals MPRF 1, and NPI documents the ULL and CPL IDs, but the exact combination NVCP writes for Off/On/Ultra is inferred [U]. Verify once: set each mode in NVCP, run `-exportCustomized`, and diff.

### 3.5 Backup and restore

1. Before the first write: run `nvidiaProfileInspector.exe -exportCustomized` (elevated). It writes `CustomProfiles_<timestamp>.nip` beside the exe (inside the Chocolatey package folder). Move it into winopt's data dir. It contains every customized profile including `Base Profile` if the user changed anything there; it only contains non-default values, which is exactly what a restore needs.
2. Keep only the `Base Profile` element for winopt's backup (or keep the whole file as a full snapshot).
3. If the export has no `Base Profile` element, the Base Profile was at defaults; the backup is then an "empty" Base Profile (`<Settings />`).
4. Restore: `nvidiaProfileInspector.exe -silentImport -replaceImport backup-base.nip`. Replace resets the Base Profile to driver defaults and re-applies the user's original non-default values. With an empty settings list it simply restores defaults.
5. After any import, re-export and diff to confirm.
6. Driver updates and the NVIDIA App can change DRS values; whether a normal (non-clean) driver update keeps Base Profile customizations is **unverified** [U]. winopt should re-check after a driver version change.

---

## 4. Measuring it per PC

### 4.1 Tools

- **PresentMon** (Intel, GameTechDev, MIT license, v2.6.0 released 2026-09-21). Console exe from GitHub releases. Uses ETW, so needs admin (or the Performance Log Users group); has `--restart_as_admin`. Key CLI: `--process_name game.exe`, `--output_file path.csv`, `--delay s`, `--timed s`, `--terminate_after_timed`, `--no_console_stats`, `--v1_metrics` / `--v2_metrics` for older column sets, `--track_pc_latency` (beta).
  - Columns (all ms): `MsBetweenPresents` (frame time at the app; the classic metric), `MsBetweenDisplayChange` (frame time as seen on screen; the right one with frame generation), `MsCPUBusy`, `MsCPUWait`, `MsGPULatency`, `MsGPUBusy`, `MsGPUWait`, `DisplayLatency` (frame start to display), `MsUntilDisplayed`, `MsRenderPresentLatency`, `MsAnimationError`, `MsClickToPhotonLatency` and `MsAllInputToPhotonLatency` (software-estimated input to display for frames that consumed a click or input), `MsPCLatency` ("PC receiving input to frame sent to display"; needs the game to emit PC Latency / PCL Stats events, i.e. Reflex SDK integration), `PresentMode`, `AllowsTearing`, `FrameType`.
- **NVIDIA FrameView**: free (not open source), built on PresentMon. Reports **PC Latency** in all games that support PCL Stats (Reflex and DLSS 4 titles, Vulkan PCL added in FrameView 1.6), plus power. Shows N/A in menus. NVIDIA defines PC Latency = input-to-frame-start + frame-start-to-present + present-to-displayed, averaged; it excludes mouse and display latency.
- **CapFrameX**: open source GUI (GitHub CXWorld/CapFrameX; GitHub reports license as "NOASSERTION", so check the LICENSE file before bundling), uses PresentMon 2.6.0 internally, has a beta PC Latency option, and an MCP/web service on localhost:1337 for automation. Good for visual comparison; winopt can call PresentMon directly instead.
- **NVIDIA Reflex Latency Analyzer**: built into some G-SYNC esports monitors (originally 360 Hz). Measures click-to-photon with a compatible mouse plugged into the monitor's USB port; any non-Bluetooth mouse gives PC+display latency. Hardware dependent, so optional.
- **Special hardware** (NVIDIA LDAT, high-speed camera plus LED mouse as used by Battle(non)sense and Blur Busters): the only true end-to-end measurements. Out of scope for winopt.

What works without special hardware: FPS, frametime percentiles, frametime variance, CPU vs GPU bound classification, `DisplayLatency`, and in Reflex/PCL games a real `MsPCLatency`. In non-PCL games, `MsClickToPhotonLatency` is an estimate and needs input during the capture; treat it as indicative, not ground truth.

### 4.2 Repeatable procedure for winopt

1. **Fix the environment**: same game, same scene (built-in benchmark preferred), same resolution and in-game settings, overlays off (except what's being tested), laptop on AC, same Windows power plan. Record driver version, GPU clocks and temps.
2. **Warm-up**: launch the game, play or loop the scene for 2 to 3 minutes so shaders are cached and the GPU reaches steady temperature. Discard this run.
3. **Capture**: `PresentMon-2.6.0-x64.exe --process_name game.exe --output_file A1.csv --delay 5 --timed 60 --terminate_after_timed --no_console_stats`. 60 s per run for a benchmark loop; 90 to 120 s for manual gameplay.
4. **Repeat**: at least 3 runs per configuration, alternating A B A B A B (baseline then preset) to spread thermal drift. Apply each profile via NPI between runs and restart the game (driver profile values are read at process start [U, standard practice]).
5. **Compute per run**, then take the **median of runs**:
   - Average FPS = frames / capture seconds (not the mean of per-frame FPS).
   - 1% low: state the definition. Use the 99th percentile frame time converted to FPS (1000 / P99 ms). Optionally also "0.1% low".
   - Frametime consistency: standard deviation of `MsBetweenPresents`, and the share of frames above 2x the median frame time (stutter count).
   - Latency: median and P95 of `MsPCLatency` when available; otherwise median `DisplayLatency` (frame start to display, a lower bound of PC latency) [U as a proxy].
   - Bound type: GPU-bound if median `MsGPUBusy` is within about 10% of median frame time, else CPU-bound (threshold is my suggestion) [U].
6. **Decide**: accept a change only if the difference exceeds the run-to-run spread (for example, the min of B runs is better than the max of A runs, or the difference exceeds 2x the standard deviation across runs). Report "no measurable difference" otherwise. This matters because, per the sources above, most settings will land in the noise.
7. **Show the user**: before vs after for average FPS, 1% low, frametime SD, stutter count, and latency, plus the GPU/CPU-bound tag.

---

## 5. Sources

1. NVIDIA Profile Inspector, official repo (README, MIT license, command line): https://github.com/Orbmu2k/nvidiaProfileInspector
2. NPI `CustomSettingNames.xml` (NPI-only setting IDs and value labels): https://github.com/Orbmu2k/nvidiaProfileInspector/blob/master/nvidiaProfileInspector/CustomSettingNames.xml
3. NPI `App.xaml.cs` (argument parsing, silent and merge/replace behaviour): https://github.com/Orbmu2k/nvidiaProfileInspector/blob/master/nvidiaProfileInspector/App.xaml.cs
4. NPI `DrsImportService.cs` (import, reset, export logic): https://github.com/Orbmu2k/nvidiaProfileInspector/blob/master/nvidiaProfileInspector/Common/DrsImportService.cs
5. NPI import model classes (`.nip` schema): https://github.com/Orbmu2k/nvidiaProfileInspector/tree/master/nvidiaProfileInspector/Common/Import
6. NPI `app.manifest` (requireAdministrator): https://github.com/Orbmu2k/nvidiaProfileInspector/blob/master/nvidiaProfileInspector/app.manifest
7. NPI releases: https://github.com/Orbmu2k/nvidiaProfileInspector/releases
8. NVIDIA NVAPI `NvApiDriverSettings.h` (setting IDs and value enums): https://github.com/NVIDIA/nvapi/blob/main/NvApiDriverSettings.h
9. Chocolatey package `nvidia-profile-inspector`: https://community.chocolatey.org/packages/nvidia-profile-inspector
10. Chocolatey package source (jtcmedia): https://github.com/jtcmedia/chocolatey-packages/tree/master/nvidia-profile-inspector
11. NVIDIA, Gamescom 2019 Game Ready Driver 436.02 (Low Latency Mode introduction): https://www.nvidia.com/en-us/geforce/news/gamescom-2019-game-ready-driver/
12. NVIDIA, Reflex Low Latency Platform (Reflex vs Ultra, Boost, latency definitions, Reflex Latency Analyzer): https://www.nvidia.com/en-us/geforce/news/reflex-low-latency-platform/
13. NVIDIA Technical Blog, Understanding and Measuring PC Latency (PCL Stats, FrameView): https://developer.nvidia.com/blog/understanding-and-measuring-pc-latency/
14. Blur Busters, G-SYNC 101: Optimal G-SYNC Settings and Conclusion: https://blurbusters.com/gsync/gsync101-input-lag-tests-and-settings/14/
15. Blur Busters, G-SYNC 101: In-game vs External FPS Limiters (measured limiter delay; comment thread on frame generation): https://blurbusters.com/gsync/gsync101-input-lag-tests-and-settings/11/
16. Blur Busters forum, "Driver 441.08: Ultra-Low Latency Now with G-SYNC Support" (user reports, unverified): https://forums.blurbusters.com/viewtopic.php?p=44689
17. TechPowerUp, GeForce 551.23 WHQL news (Ultra Low Latency for DX12): https://www.techpowerup.com/318252/nvidia-releases-geforce-551-23-whql-supports-rtx-4070-ti-super-rtx-video-hdr
18. DSOGaming, GeForce 496.13 adds Shader Cache Size setting: https://www.dsogaming.com/news/nvidia-geforce-496-13-whql-driver-released-adds-shader-cache-size-setting
19. TheFPSReview, NVIDIA GeForce Driver Power Mode Settings Compared (2019, measured): https://www.thefpsreview.com/2019/12/04/nvidia-geforce-driver-power-mode-settings-compared/11/
20. TechPowerUp, NVIDIA Resizable BAR Performance Test, 22 games (measured, includes NPI rBAR IDs): https://www.techpowerup.com/review/nvidia-pci-express-resizable-bar-performance-test/27.html
21. Tom's Hardware summary of Battle(non)sense Reflex / Low Latency / frame cap test (secondary; I could not read the article body or the video): https://www.tomshardware.com/news/reflex-destroys-radeon-boost-low-latency
22. PCGamingWiki community note on NVIDIA limiter latency vs RTSS, citing Battle(non)sense (secondary): https://community.pcgamingwiki.com/topic/6247-nvidia-profile-inspector-page-error-about-nvidias-fps-limiter-latency-note
23. NVIDIA, DLSS 4.5 Super Resolution Available Now (presets K/L/M, FP8 cost on RTX 20/30): https://www.nvidia.com/en-us/geforce/news/dlss-4-5-super-resolution-available-now/
24. NVIDIA, NVIDIA App update: DLSS overrides (not read in full): https://www.nvidia.com/en-us/geforce/news/gfecnt/20251/nvidia-app-update-dlss-overrides-and-more/
25. PresentMon console application README (CLI and CSV columns): https://github.com/GameTechDev/PresentMon/blob/main/README-ConsoleApplication.md
26. CapFrameX repo: https://github.com/CXWorld/CapFrameX
27. NVIDIA FrameView 1.7 user guide (PDF): https://images.nvidia.com/content/geforce/technologies/frameview/frameview-1-7-user-guide-web-version.pdf
28. NVIDIA FrameView product page: https://www.nvidia.com/en-us/geforce/technologies/frameview/
29. Guru3D, NVIDIA Adds Auto Shader Compilation Beta (secondary): https://www.guru3d.com/story/nvidia-adds-auto-shader-compilation-beta-to-cut-load-times/
30. Guru3D forum, MSI AB / RTSS development news thread (RTSS limiter modes; forum): https://forums.guru3d.com/threads/msi-ab-rtss-development-news-thread.412822/page-164
31. TUXEDO Computers, What is a MUX switch (vendor explainer, secondary): https://www.tuxedocomputers.com/en/What-is-a-MUX-Switch-and-what-is-it-good-for-.tuxedo
32. Laptop Mag, The MUX Switch is overhyped (secondary, latency estimate): https://www.laptopmag.com/features/the-mux-switch-is-overhyped-heres-why
33. Blur Busters forum, threaded optimization thread (folklore, unverified): https://forums.blurbusters.com/viewtopic.php?p=98583
34. Guru3D forum, "Nvidia Has a Driver Overhead Problem" thread with threaded optimization A/B (folklore, unverified): https://forums.guru3d.com/goto/post?id=5898570
35. Beyond3D, anisotropic filtering performance reviews (historical; the 7800 GTX 8% figure came from a search snippet and I did not confirm which Beyond3D page it is on): https://beyond3d.com/content/reviews/36/28
36. AnandTech, GeForce 8800 (G80) texture filtering (historical): https://www.anandtech.com/show/2116/15

### Not verified / gaps

- Battle(non)sense, Hardware Unboxed, Gamers Nexus, Digital Foundry and Igor's Lab primary videos/articles on these specific driver settings: I could not retrieve their numbers directly; only secondary summaries (21, 22) are cited.
- No controlled modern benchmark found for Threaded optimization, Texture filtering quality, Anisotropic filtering cost, Shader cache size, or Preferred refresh rate.
- Exact NVCP-to-DRS write pattern for Low Latency Mode Off/On/Ultra (verify with an export diff).
- Meaning of rBAR v2, rBAR flags `0x000F00BE`/`0x000F00BF`, and the Intel CPU exclusion mask.
- Whether driver updates preserve Base Profile customizations, and NPI's process exit code on failure.
