# Measuring

## Frame statistics

One method everywhere (live monitor, benchmarks, reports), defined at the top of `stats.rs`:

- **Average FPS** = frames ÷ elapsed time = `1000 · n / Σ frame_time_ms`. Not the mean of per-frame FPS values, which over-weights fast frames.
- **Percentiles** (p50, p90, p95, p99, p99.9) of frame time: linear interpolation between closest ranks.
- **1% low / 0.1% low FPS**: average FPS over the slowest 1% / 0.1% of frames. Reported only with enough frames (300 for the 1% low, 3000 for the 0.1% low); otherwise the value is absent and the reason is shown. Never estimated.
- **Percentile FPS** (`1000 / p99`, `1000 / p99.9`) is stored separately and labelled separately, because some tools call that "1% low".
- **Consistency** = p50 ÷ p99 frame time, in percent.
- **Stutter frames**: longer than both twice the median and the median + 4 ms.

Displayed figures use frames as shown on screen. If the source has no display timing, presented figures are used and labelled as such.

## Diagnostics

`diagnostics.rs` is a deterministic rule engine: the same inputs give the same findings in the same order, and every finding lists the measurements it rests on and a confidence.

With PresentMon frame data:

- GPU busy ÷ frame time ≥ 0.90: GPU-bound.
- GPU busy ÷ frame time ≤ 0.75 and CPU busy ÷ frame time ≥ 0.75: CPU-bound.
- Average FPS sitting on the refresh rate or a common cap with steady frame times: a limiter or V-Sync is in charge.

Without frame data only low-confidence hints are given (for example "one core is full while the GPU has headroom"), and the report says which data it used. A busy processor alone is never reported as a CPU bottleneck. Other rules cover memory pressure, VRAM pressure, temperature and power limits when a vendor API reports them, refresh rate below the display's maximum, power plan, and background load. Thresholds are constants at the top of the file.

## Benchmarks

A benchmark is a recording of the frames Live Monitor measures. Start it from Live Monitor or Benchmark History (or let a game profile record the whole session). Stopping the monitor, or the game exiting, saves what was captured.

A session stores:

- the game, executable, start time and length (from the frames, not the wall clock);
- displayed and presented frame statistics, CPU / GPU busy and wait per frame, display latency, dropped and generated frame shares;
- average and peak GPU temperature, GPU load and power, CPU load and temperature, peak VRAM, average RAM (when a vendor API reports them);
- the display mode, CPU, GPU, driver and Windows version;
- which Arcadia optimisations and which game profile were active;
- notes on anything missing or unreliable;
- a downsampled frame-time series for the chart (spikes are kept).

The display mode is the desktop's. The game's own render resolution and graphics settings are not visible to PresentMon; name the run to record them.

### Comparing

Pick two or more sessions; the earliest is the baseline. Each measurement shows the value, the absolute difference and the percentage difference. A comparison is flagged when:

- a run is shorter than 30 seconds, or hit the recording size limit;
- the games, display resolution, refresh rate or hardware differ;
- capture lengths differ a lot (the runs may not cover the same part of the game);
- GPU temperature differs by 8 °C or more, or CPU temperature by 10 °C or more;
- overall CPU load differs by 15 points or more while the game's own per-frame CPU work is similar (other programs were probably busy);
- the share of generated frames differs.

The summary states measured differences only. A difference under 3% is called run-to-run variation. FrameForge never says a change caused a difference: it cannot tell whether two runs covered the same scene, so it asks for repeated runs ("Repeat" records the same test again under the same name).

### Export

CSV (one row per session, every stored measurement, formula-injection safe) or JSON (sessions plus the comparison when there are two or more).
