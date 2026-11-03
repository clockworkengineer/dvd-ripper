use criterion::{Criterion, black_box, criterion_group, criterion_main};
use dvd_ripper::domain::entities::disc::DvdTitleInfo;
use dvd_ripper::domain::heuristics::cluster_detector::rank_best_title_from_slice;
use dvd_ripper::domain::heuristics::tv_detector::detect_tv_episodes_from_titles;
use dvd_ripper::infrastructure::ffmpeg::progress_parser::parse_ffmpeg_progress_bytes;
use dvd_ripper::iso::copy_sectors_with_progress;
use dvd_ripper::utils::parse_ffmpeg_progress_line;
use std::io::Cursor;

fn bench_ffmpeg_progress_parser(c: &mut Criterion) {
    let raw_line = "frame= 1250 fps= 48.5 q=28.0 size= 15360kB time=00:01:30.50 bitrate=1389.2kbits/s speed= 1.95x";
    let raw_bytes = raw_line.as_bytes();

    let mut group = c.benchmark_group("ffmpeg_progress_parser");

    group.bench_function("parse_ffmpeg_progress_line_str", |b| {
        b.iter(|| {
            let res = parse_ffmpeg_progress_line(black_box(raw_line));
            black_box(res);
        })
    });

    group.bench_function("parse_ffmpeg_progress_bytes_zero_alloc", |b| {
        b.iter(|| {
            let res = parse_ffmpeg_progress_bytes(black_box(raw_bytes));
            black_box(res);
        })
    });

    group.finish();
}

fn bench_sector_copy_throughput(c: &mut Criterion) {
    let total_size = 4 * 1024 * 1024;
    let data = vec![0xAAu8; total_size];

    c.bench_function("copy_sectors_with_progress_4mb", |b| {
        b.iter(|| {
            let reader = Cursor::new(&data);
            let mut writer = Cursor::new(Vec::with_capacity(total_size));
            let copied = copy_sectors_with_progress(
                black_box(reader),
                black_box(&mut writer),
                black_box(Some(total_size as u64)),
                black_box(None),
                |_, _, _| {},
            )
            .unwrap();
            black_box(copied);
        })
    });
}

fn bench_heuristics(c: &mut Criterion) {
    let mut titles = Vec::with_capacity(100);
    for i in 1..=100 {
        let duration = if i % 10 == 0 {
            5400.0
        } else if i % 3 == 0 {
            2700.0
        } else {
            120.0
        };
        titles.push(DvdTitleInfo {
            title_num: i,
            duration_secs: duration,
        });
    }

    let mut group = c.benchmark_group("heuristics");

    group.bench_function("cluster_detector_theatrical_ranking", |b| {
        b.iter(|| {
            let res = rank_best_title_from_slice(black_box(&titles), black_box(Some(5400.0)));
            black_box(res);
        })
    });

    group.bench_function("tv_detector_episode_detection", |b| {
        b.iter(|| {
            let res = detect_tv_episodes_from_titles(
                black_box(&titles),
                black_box("Stargate SG-1"),
                1,
                1,
                None,
            );
            black_box(res);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_ffmpeg_progress_parser,
    bench_sector_copy_throughput,
    bench_heuristics
);
criterion_main!(benches);
