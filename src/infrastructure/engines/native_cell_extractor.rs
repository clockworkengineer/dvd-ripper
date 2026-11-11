//! Native Rust DVD-Video IFO Cell-Sector Extractor (`NativeIfoTranscoder`).
//!
//! Reads the binary `VIDEO_TS.IFO` and `VTS_xx_0.IFO` Program Chain (`VTS_PGCITI`) and
//! Cell Playback Information Tables (`C_PBIT`) to extract only the exact `(first_sector..=last_sector)`
//! 2048-byte sector spans belonging to a title (or chapter range) directly from `VTS_xx_1..9.VOB`.
//!
//! This completely bypasses `ffmpeg -f dvdvideo` demuxer bugs, Sony ARccOS / Disney bad-sector
//! gap traps, and multi-PGC navigation confusion.

use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{Context, Result, anyhow};

use crate::domain::ports::transcoder::{
    ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder,
};
use crate::infrastructure::optical::ifo_parser::resolve_vts_ttn_to_pgcn;

/// Size of a standard DVD-Video logical sector in bytes.
pub const DVD_SECTOR_BYTES: u64 = 2048;

/// Planned sector extraction layout for a single DVD title (or chapter range) resolved from IFO tables.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeCellPlan {
    /// 1-based Video Title Set number (`VTS_xx`).
    pub vtsn: u8,
    /// 1-based Program Chain number (`PGCN`) resolved via `VTS_PTT_SRPT`.
    pub pgcn: usize,
    /// Ordered `(first_sector, last_sector)` inclusive 2048-byte sector spans within the VTS VOB set.
    pub cell_sectors: Vec<(u32, u32)>,
    /// Total number of 2048-byte sectors across all selected cells.
    pub total_sectors: u64,
    /// Estimated playback duration in seconds.
    pub duration_secs: f64,
}

/// Resolves the exact [`NativeCellPlan`] for `title_num` (1-based) and optional `(start_chapter, end_chapter)` (1-based)
/// by parsing `VIDEO_TS.IFO` and `VTS_xx_0.IFO`.
pub fn build_native_cell_plan(
    dvd_path: &Path,
    title_num: u32,
    chapter_range: Option<(u32, u32)>,
) -> Option<NativeCellPlan> {
    if title_num == 0 {
        return None;
    }

    let ifo_dirs = [
        dvd_path.join("VIDEO_TS"),
        dvd_path.join("video_ts"),
        dvd_path.to_path_buf(),
    ];

    let vmg_candidates = [
        "VIDEO_TS.IFO",
        "VIDEO_TS.BUP",
        "video_ts.ifo",
        "video_ts.bup",
        "VIDEO_TS.ifo",
        "VIDEO_TS.bup",
    ];

    let mut vtsn_ttn = None;
    let mut resolved_ifo_dir = dvd_path.to_path_buf();

    for dir in &ifo_dirs {
        for name in &vmg_candidates {
            let candidate = dir.join(name);
            if let Ok(data) = std::fs::read(&candidate)
                && data.len() >= 0xC8
            {
                let sector_offset = u32::from_be_bytes([data[0xc4], data[0xc5], data[0xc6], data[0xc7]]);
                let byte_offset = (sector_offset as usize) * 2048;
                if byte_offset + 8 <= data.len() {
                    let nr_titles = u16::from_be_bytes([data[byte_offset], data[byte_offset + 1]]) as usize;
                    if let Some(idx) = (title_num as usize).checked_sub(1)
                        && idx < nr_titles
                        && byte_offset + 8 + (idx + 1) * 12 <= data.len()
                    {
                        let entry_off = byte_offset + 8 + idx * 12;
                        let vtsn = data[entry_off + 6];
                        let vts_ttn = data[entry_off + 7];
                        if vtsn > 0 && vts_ttn > 0 {
                            vtsn_ttn = Some((vtsn, vts_ttn));
                            resolved_ifo_dir = dir.clone();
                            break;
                        }
                    }
                }
            }
        }
        if vtsn_ttn.is_some() {
            break;
        }
    }

    // Direct fallback for copy-protected or bad-sector root IFOs (e.g. Sony ARccOS / Disney):
    // Map title_num directly to VTS_xx_0.IFO (title 1 -> VTS 1, etc.)
    let (vtsn, vts_ttn) = vtsn_ttn.unwrap_or_else(|| (title_num.clamp(1, 99) as u8, 1));

    let vts_names = [
        format!("VTS_{:02}_0.IFO", vtsn),
        format!("VTS_{:02}_0.BUP", vtsn),
        format!("vts_{:02}_0.ifo", vtsn),
        format!("vts_{:02}_0.bup", vtsn),
        format!("VTS_{:02}_0.ifo", vtsn),
        format!("VTS_{:02}_0.bup", vtsn),
    ];

    let mut vts_data_opt = None;
    for dir in &[&resolved_ifo_dir, &dvd_path.join("VIDEO_TS"), &dvd_path.join("video_ts"), &dvd_path.to_path_buf()] {
        for name in &vts_names {
            let p = dir.join(name);
            if let Ok(d) = std::fs::read(&p)
                && d.len() > 0xCF
            {
                vts_data_opt = Some(d);
                break;
            }
        }
        if vts_data_opt.is_some() {
            break;
        }
    }
    let vts_data = vts_data_opt?;
    if vts_data.len() <= 0xCF {
        return None;
    }

    let pgciti_sec = u32::from_be_bytes([
        vts_data[0xCC],
        vts_data[0xCD],
        vts_data[0xCE],
        vts_data[0xCF],
    ]);
    let pgciti_off = (pgciti_sec as usize) * 2048;
    let pgcn = resolve_vts_ttn_to_pgcn(&vts_data, vts_ttn);
    if pgciti_off + 8 + pgcn * 8 > vts_data.len() {
        return None;
    }

    let pgc_entry_off = pgciti_off + 8 + (pgcn - 1) * 8;
    let pgc_start_rel = u32::from_be_bytes([
        vts_data[pgc_entry_off + 4],
        vts_data[pgc_entry_off + 5],
        vts_data[pgc_entry_off + 6],
        vts_data[pgc_entry_off + 7],
    ]);
    let pgc_off = pgciti_off + (pgc_start_rel as usize);
    if pgc_off + 0xEC > vts_data.len() {
        return None;
    }

    let nr_programs = vts_data[pgc_off + 2] as usize;
    let nr_cells = vts_data[pgc_off + 3] as usize;
    let h_bcd = vts_data[pgc_off + 4];
    let m_bcd = vts_data[pgc_off + 5];
    let s_bcd = vts_data[pgc_off + 6];
    let h = (((h_bcd >> 4) * 10) + (h_bcd & 0x0F)) as f64;
    let m = (((m_bcd >> 4) * 10) + (m_bcd & 0x0F)) as f64;
    let s = (((s_bcd >> 4) * 10) + (s_bcd & 0x0F)) as f64;
    let duration_secs = h * 3600.0 + m * 60.0 + s;

    let pg_map_rel =
        u16::from_be_bytes([vts_data[pgc_off + 0xE6], vts_data[pgc_off + 0xE7]]) as usize;
    let c_pbit_rel =
        u16::from_be_bytes([vts_data[pgc_off + 0xE8], vts_data[pgc_off + 0xE9]]) as usize;
    let c_pbit_off = pgc_off + c_pbit_rel;
    if c_pbit_rel == 0 || nr_cells == 0 || c_pbit_off + nr_cells * 24 > vts_data.len() {
        return None;
    }

    let mut all_cells = Vec::with_capacity(nr_cells);
    for c in 0..nr_cells {
        let c_off = c_pbit_off + c * 24;
        let first_sec = u32::from_be_bytes([
            vts_data[c_off + 8],
            vts_data[c_off + 9],
            vts_data[c_off + 10],
            vts_data[c_off + 11],
        ]);
        let last_sec = u32::from_be_bytes([
            vts_data[c_off + 16],
            vts_data[c_off + 17],
            vts_data[c_off + 18],
            vts_data[c_off + 19],
        ]);
        if last_sec >= first_sec {
            all_cells.push((first_sec, last_sec));
        }
    }

    // Slice by chapter range if requested and PG_MAP is valid
    let selected_cells = if let Some((start_ch, end_ch)) = chapter_range {
        let pg_map_off = pgc_off + pg_map_rel;
        if pg_map_rel > 0 && nr_programs > 0 && pg_map_off + nr_programs <= vts_data.len() {
            let s_prog = (start_ch.max(1) as usize).saturating_sub(1).min(nr_programs - 1);
            let e_prog = (end_ch.max(start_ch) as usize)
                .saturating_sub(1)
                .min(nr_programs - 1);
            let start_cell_idx = (vts_data[pg_map_off + s_prog] as usize)
                .saturating_sub(1)
                .min(all_cells.len().saturating_sub(1));
            let end_cell_idx = if e_prog + 1 < nr_programs {
                (vts_data[pg_map_off + e_prog + 1] as usize)
                    .saturating_sub(1)
                    .max(start_cell_idx + 1)
                    .min(all_cells.len())
            } else {
                all_cells.len()
            };
            all_cells[start_cell_idx..end_cell_idx].to_vec()
        } else {
            all_cells
        }
    } else {
        all_cells
    };

    if selected_cells.is_empty() {
        return None;
    }

    let total_sectors: u64 = selected_cells
        .iter()
        .map(|&(first, last)| (last as u64).saturating_sub(first as u64) + 1)
        .sum();

    Some(NativeCellPlan {
        vtsn,
        pgcn,
        cell_sectors: selected_cells,
        total_sectors,
        duration_secs,
    })
}

/// Locates the ordered list of title VOB files (`VTS_xx_1.VOB` .. `VTS_xx_9.VOB`) for a Video Title Set.
pub fn locate_vts_vob_files(dvd_path: &Path, vtsn: u8) -> Vec<(PathBuf, u64)> {
    let dirs = [
        dvd_path.join("VIDEO_TS"),
        dvd_path.join("video_ts"),
        dvd_path.to_path_buf(),
    ];

    let mut result = Vec::new();
    for dir in &dirs {
        if !dir.exists() {
            continue;
        }
        for part in 1..=9u8 {
            let candidates = [
                dir.join(format!("VTS_{:02}_{}.VOB", vtsn, part)),
                dir.join(format!("vts_{:02}_{}.vob", vtsn, part)),
                dir.join(format!("VTS_{:02}_{}.vob", vtsn, part)),
            ];
            if let Some(p) = candidates.into_iter().find(|p| p.exists())
                && let Ok(meta) = std::fs::metadata(&p)
                && meta.len() > 0
            {
                let sectors = meta.len() / DVD_SECTOR_BYTES;
                result.push((p, sectors));
            }
        }
        if !result.is_empty() {
            break;
        }
    }
    result
}

/// Extracts the exact PGC cells defined in [`NativeCellPlan`] from the VTS `.VOB` files on `dvd_path`
/// into `output_vob_path`, skipping unreferenced bad sectors and reporting live progress.
pub fn extract_pgc_cells_to_file(
    dvd_path: &Path,
    plan: &NativeCellPlan,
    output_vob_path: &Path,
    observer: Option<&dyn ProgressObserver>,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<u64> {
    let vob_parts = locate_vts_vob_files(dvd_path, plan.vtsn);
    if vob_parts.is_empty() {
        return Err(anyhow!(
            "No VTS_{:02}_1..9.VOB files found on {}",
            plan.vtsn,
            dvd_path.display()
        ));
    }

    if let Some(parent) = output_vob_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let out_file = File::create(output_vob_path).with_context(|| {
        format!(
            "Failed to create native cell extraction output {}",
            output_vob_path.display()
        )
    })?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, out_file);

    // Build cumulative sector start table for VTS_xx_1..9.VOB
    let mut part_spans = Vec::with_capacity(vob_parts.len());
    let mut cumulative_sec = 0u64;
    for (path, sector_count) in vob_parts {
        let file = File::open(&path)
            .with_context(|| format!("Failed to open VOB part {}", path.display()))?;
        part_spans.push((cumulative_sec, cumulative_sec + sector_count, file));
        cumulative_sec += sector_count;
    }

    let total_sectors = plan.total_sectors.max(1);
    let mut copied_sectors = 0u64;
    let start_instant = Instant::now();
    // 32 sectors = 64 KiB read chunk
    let mut chunk_buf = vec![0u8; 32 * (DVD_SECTOR_BYTES as usize)];

    if let Some(obs) = observer {
        obs.on_log(&format!(
            "[Native IFO Extractor] Extracting VTS_{:02} PGC #{} ({} cells, {:.1} MB)...",
            plan.vtsn,
            plan.pgcn,
            plan.cell_sectors.len(),
            (plan.total_sectors * DVD_SECTOR_BYTES) as f64 / 1_048_576.0
        ));
    }

    for &(first_sec, last_sec) in &plan.cell_sectors {
        let mut cur_sec = first_sec as u64;
        let end_sec_exclusive = (last_sec as u64) + 1;

        while cur_sec < end_sec_exclusive {
            if let Some(flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                drop(writer);
                let _ = std::fs::remove_file(output_vob_path);
                return Err(anyhow!("Native IFO cell extraction cancelled by user."));
            }

            // Find the VOB part containing `cur_sec`
            let Some((_, part_end_sec, part_file)) = part_spans
                .iter_mut()
                .find(|(s, e, _)| cur_sec >= *s && cur_sec < *e)
            else {
                break;
            };

            let part_start_sec = *part_end_sec
                - (part_file.metadata().map(|m| m.len()).unwrap_or(0) / DVD_SECTOR_BYTES);
            let rel_sec = cur_sec.saturating_sub(part_start_sec);
            let max_in_part = (*part_end_sec).min(end_sec_exclusive) - cur_sec;
            let sectors_to_read = max_in_part.min(32) as usize;
            let bytes_to_read = sectors_to_read * (DVD_SECTOR_BYTES as usize);

            if part_file
                .seek(SeekFrom::Start(rel_sec * DVD_SECTOR_BYTES))
                .is_ok()
            {
                match part_file.read_exact(&mut chunk_buf[..bytes_to_read]) {
                    Ok(()) => {
                        writer.write_all(&chunk_buf[..bytes_to_read])?;
                    }
                    Err(_) => {
                        // Bad sector fallback: read sector-by-sector and skip unreadable damaged sectors
                        for s_offset in 0..sectors_to_read {
                            let single_pos = (rel_sec + s_offset as u64) * DVD_SECTOR_BYTES;
                            if part_file.seek(SeekFrom::Start(single_pos)).is_ok()
                                && part_file
                                    .read_exact(&mut chunk_buf[..DVD_SECTOR_BYTES as usize])
                                    .is_ok()
                            {
                                writer.write_all(&chunk_buf[..DVD_SECTOR_BYTES as usize])?;
                            }
                        }
                    }
                }
            }

            cur_sec += sectors_to_read as u64;
            copied_sectors += sectors_to_read as u64;

            if copied_sectors.is_multiple_of(512) || copied_sectors >= total_sectors {
                let pct = ((copied_sectors as f64 / total_sectors as f64) * 100.0).clamp(0.0, 99.5);
                let elapsed = start_instant.elapsed().as_secs_f64().max(0.001);
                let mb_s = ((copied_sectors * DVD_SECTOR_BYTES) as f64 / 1_048_576.0) / elapsed;
                let speed_x = mb_s / 1.35; // 1x DVD speed = ~1.35 MB/s
                if let Some(obs) = observer {
                    obs.on_progress(pct, "N/A", &format!("{:.1}x ({:.1} MB/s)", speed_x, mb_s));
                }
            }
        }
    }

    writer.flush()?;
    Ok(copied_sectors * DVD_SECTOR_BYTES)
}

/// Native Rust IFO Cell-Sector [`Transcoder`] implementation.
///
/// Extracts exact PGC cells directly from the optical disc via [`extract_pgc_cells_to_file`]
/// and optionally remuxes or transcodes the clean stream into `.mkv` / `.mp4` using `ffmpeg`.
pub struct NativeIfoTranscoder {
    ffmpeg_path: String,
    options: TranscodeOptions,
}

impl NativeIfoTranscoder {
    pub fn new(ffmpeg_path: impl Into<String>, options: TranscodeOptions) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
            options,
        }
    }
}

impl Transcoder for NativeIfoTranscoder {
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        self.transcode_chapter_range(job, None, observer, cancel_flag)
    }

    fn transcode_chapter_range(
        &self,
        job: &TranscodeJob,
        chapter_range: Option<(u32, u32)>,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        let range = chapter_range.or(
            match (self.options.start_chapter, self.options.end_chapter) {
                (Some(s), Some(e)) => Some((s, e)),
                (Some(s), None) => Some((s, 99)),
                (None, Some(e)) => Some((1, e)),
                (None, None) => None,
            },
        );

        let title_num = if job.title_number == 0 {
            1
        } else {
            job.title_number
        };
        let plan = build_native_cell_plan(job.source_dvd, title_num, range).ok_or_else(|| {
            anyhow!(
                "Native IFO extractor could not locate valid PGC cell table for Title #{} on {}",
                title_num,
                job.source_dvd.display()
            )
        })?;

        let ext = job
            .output_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mpg")
            .to_lowercase();

        // Direct pure-Rust extraction when output is MPEG-PS (.mpg / .vob) and transcoding is disabled
        if !self.options.transcode && (ext == "mpg" || ext == "mpeg" || ext == "vob") {
            extract_pgc_cells_to_file(
                job.source_dvd,
                &plan,
                job.output_path,
                observer,
                cancel_flag.as_ref(),
            )?;
            if let Some(obs) = observer {
                obs.on_progress(100.0, "N/A", "Done");
            }
            return Ok(());
        }

        // Otherwise, extract clean PGC cells to a temporary VOB and mux/transcode to MKV/MP4
        let temp_vob = job.output_path.with_extension("native_cells.tmp.vob");
        let extract_res = extract_pgc_cells_to_file(
            job.source_dvd,
            &plan,
            &temp_vob,
            observer,
            cancel_flag.as_ref(),
        );
        if let Err(e) = extract_res {
            let _ = std::fs::remove_file(&temp_vob);
            return Err(e);
        }

        if let Some(obs) = observer {
            obs.on_log(&format!(
                "[Native IFO Extractor] Clean PGC stream extracted; muxing to {}...",
                job.output_path.display()
            ));
        }

        let mut cmd = Command::new(&self.ffmpeg_path);
        crate::utils::configure_silent_command(&mut cmd);
        cmd.stdin(std::process::Stdio::null())
            .arg("-nostdin")
            .arg("-y")
            .arg("-fflags")
            .arg("+genpts+igndts")
            .arg("-i")
            .arg(&temp_vob)
            .arg("-map")
            .arg("0:v:0?")
            .arg("-map")
            .arg(if self.options.all_audio {
                "0:a?"
            } else {
                "0:a:0?"
            });

        if self.options.subtitles && ext == "mkv" {
            cmd.arg("-map").arg("0:s?");
        }

        if !self.options.transcode || self.options.codec.eq_ignore_ascii_case("copy") {
            cmd.arg("-c").arg("copy");
        } else {
            let vcodec = crate::ffmpeg::resolve_video_codec_name(&self.options.codec);
            cmd.arg("-c:v")
                .arg(vcodec)
                .arg("-preset")
                .arg(&self.options.preset)
                .arg("-crf")
                .arg(self.options.crf.unwrap_or(20).to_string())
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("192k");
        }

        cmd.arg(job.output_path);
        let status = cmd.status();
        let _ = std::fs::remove_file(&temp_vob);

        let exit = status.context("Failed to execute FFmpeg container mux after native cell extraction")?;
        if !exit.success() {
            return Err(anyhow!(
                "Container muxing exited with non-zero status: {}",
                exit
            ));
        }

        if let Some(obs) = observer {
            obs.on_progress(100.0, "N/A", "Done");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_cell_plan_and_extraction_synthetic_disc() {
        let tmp = std::env::temp_dir().join(format!(
            "dvd_ripper_native_cell_test_{}",
            std::process::id()
        ));
        let video_ts = tmp.join("VIDEO_TS");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&video_ts).unwrap();

        // Build synthetic VIDEO_TS.IFO with 1 title in VTS 1, TTN 1
        let mut vmg = vec![0u8; 4096];
        vmg[0xc4..0xc8].copy_from_slice(&1u32.to_be_bytes());
        vmg[2048..2050].copy_from_slice(&1u16.to_be_bytes());
        let entry = 2048 + 8;
        vmg[entry + 1] = 1;
        vmg[entry + 6] = 1; // VTS 1
        vmg[entry + 7] = 1; // TTN 1
        std::fs::write(video_ts.join("VIDEO_TS.IFO"), &vmg).unwrap();

        // Build synthetic VTS_01_0.IFO with 2 cells: sectors 1..=2 and 4..=5 (skipping sector 0 and 3!)
        let mut vts = vec![0u8; 6144];
        vts[0xCC..0xD0].copy_from_slice(&1u32.to_be_bytes());
        let pgciti = 2048;
        vts[pgciti..pgciti + 2].copy_from_slice(&1u16.to_be_bytes());
        let ptr = pgciti + 8;
        vts[ptr + 4..ptr + 8].copy_from_slice(&16u32.to_be_bytes());
        let pgc = pgciti + 16;
        vts[pgc + 2] = 2; // 2 programs
        vts[pgc + 3] = 2; // 2 cells
        vts[pgc + 4] = 0x00;
        vts[pgc + 5] = 0x42; // 42 mins
        vts[pgc + 6] = 0x00;
        let pg_map_rel: u16 = 0xEC;
        let c_pbit_rel: u16 = 0xF0;
        vts[pgc + 0xE6..pgc + 0xE8].copy_from_slice(&pg_map_rel.to_be_bytes());
        vts[pgc + 0xE8..pgc + 0xEA].copy_from_slice(&c_pbit_rel.to_be_bytes());
        vts[pgc + (pg_map_rel as usize)] = 1; // Program 1 -> Cell 1
        vts[pgc + (pg_map_rel as usize) + 1] = 2; // Program 2 -> Cell 2

        let c1 = pgc + (c_pbit_rel as usize);
        vts[c1 + 8..c1 + 12].copy_from_slice(&1u32.to_be_bytes());
        vts[c1 + 16..c1 + 20].copy_from_slice(&2u32.to_be_bytes());
        let c2 = c1 + 24;
        vts[c2 + 8..c2 + 12].copy_from_slice(&4u32.to_be_bytes());
        vts[c2 + 16..c2 + 20].copy_from_slice(&5u32.to_be_bytes());
        std::fs::write(video_ts.join("VTS_01_0.IFO"), &vts).unwrap();

        // Create VTS_01_1.VOB with 6 sectors (0..=5), each filled with its sector index byte
        let mut vob_bytes = Vec::with_capacity(6 * 2048);
        for s in 0..6u8 {
            vob_bytes.extend(std::iter::repeat_n(s, 2048));
        }
        std::fs::write(video_ts.join("VTS_01_1.VOB"), &vob_bytes).unwrap();

        let plan = build_native_cell_plan(&tmp, 1, None).unwrap();
        assert_eq!(plan.vtsn, 1);
        assert_eq!(plan.cell_sectors, vec![(1, 2), (4, 5)]);
        assert_eq!(plan.total_sectors, 4);

        // Chapter 2 only -> should select Cell 2 ((4, 5))
        let ch2_plan = build_native_cell_plan(&tmp, 1, Some((2, 2))).unwrap();
        assert_eq!(ch2_plan.cell_sectors, vec![(4, 5)]);
        assert_eq!(ch2_plan.total_sectors, 2);

        let out_vob = tmp.join("extracted.mpg");
        let bytes_written = extract_pgc_cells_to_file(&tmp, &plan, &out_vob, None, None).unwrap();
        assert_eq!(bytes_written, 4 * 2048);
        let extracted = std::fs::read(&out_vob).unwrap();
        assert_eq!(extracted[0], 1);
        assert_eq!(extracted[2048], 2);
        assert_eq!(extracted[4096], 4);
        assert_eq!(extracted[6144], 5);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_native_cell_plan_edge_cases() {
        assert!(build_native_cell_plan(Path::new("Z:\\nonexistent"), 0, None).is_none());
        assert!(build_native_cell_plan(Path::new("Z:\\nonexistent"), 1, None).is_none());

        // Corrupt small IFO
        let tmp = std::env::temp_dir().join(format!("ifo_corrupt_{}", std::process::id()));
        let video_ts = tmp.join("VIDEO_TS");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&video_ts).unwrap();
        std::fs::write(video_ts.join("VIDEO_TS.IFO"), [0u8; 50]).unwrap();
        assert!(build_native_cell_plan(&tmp, 1, None).is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_extract_pgc_cells_empty_and_cancelled() {
        let empty_plan = NativeCellPlan {
            vtsn: 1,
            pgcn: 1,
            cell_sectors: vec![],
            total_sectors: 0,
            duration_secs: 0.0,
        };
        let out = std::env::temp_dir().join("empty_out.mpg");
        let res = extract_pgc_cells_to_file(Path::new("."), &empty_plan, &out, None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("No VTS_01_1..9.VOB files found"));

        // Test cancel flag with valid dummy VOB
        let tmp = std::env::temp_dir().join(format!("vob_cancel_test_{}", std::process::id()));
        let video_ts = tmp.join("VIDEO_TS");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&video_ts).unwrap();
        std::fs::write(video_ts.join("VTS_01_1.VOB"), [0u8; 4096]).unwrap();

        let plan_with_cells = NativeCellPlan {
            vtsn: 1,
            pgcn: 1,
            cell_sectors: vec![(0, 1)],
            total_sectors: 2,
            duration_secs: 10.0,
        };
        let flag = Arc::new(AtomicBool::new(true));
        let cancel_out = tmp.join("cancel_out.mpg");
        let res_cancel = extract_pgc_cells_to_file(&tmp, &plan_with_cells, &cancel_out, None, Some(&flag));
        assert!(res_cancel.is_err());
        assert!(res_cancel.unwrap_err().to_string().contains("cancelled by user"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_native_ifo_transcoder_construction_and_transcode_err() {
        let transcoder = NativeIfoTranscoder::new("ffmpeg", TranscodeOptions::default());
        let job = TranscodeJob {
            source_dvd: Path::new("Z:\\nonexistent_dvd_source"),
            output_path: Path::new("out.mkv"),
            title_number: 1,
            display_title: "Title 1",
            expected_duration_secs: Some(3600.0),
        };
        let res = transcoder.transcode(&job, None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("could not locate valid PGC cell table"));
    }
}

