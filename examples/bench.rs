//! Write/read throughput benchmark over synthetic paired reads (for backend comparisons).
//! `cargo run --release --example bench -- -n 2000000 -t 8`
use std::{fs::File, io::BufWriter, time::Instant};

use anyhow::Result;
use binseq::SequencingRecordBuilder;
use binseq::{
    BinseqReader, BinseqRecord, BinseqWriterBuilder, ParallelProcessor, ParallelReader,
    write::Format,
};
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value_t = 1_000_000)]
    n: usize,
    #[clap(short, long, default_value_t = 8)]
    threads: usize,
    #[clap(short, long, default_value_t = 3)]
    reps: usize,
    #[clap(short, long, default_value_t = 3)]
    level: i32,
    /// only run this format (cbq|vbq)
    #[clap(short, long)]
    format: Option<String>,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[derive(Clone, Default)]
struct Touch(usize);
impl ParallelProcessor for Touch {
    fn process_record<R: BinseqRecord>(&mut self, r: R) -> binseq::Result<()> {
        // touch every column so lazy (per-column) decompression is paid for
        self.0 += r.sseq().len() + r.xseq().len() + r.squal().len() + r.sheader().len();
        Ok(())
    }
}

fn main() -> Result<()> {
    let a = Args::parse();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let genome: Vec<u8> = (0..4_000_000)
        .map(|_| b"ACGT"[(rng.next() & 3) as usize])
        .collect();
    let rl = 150;
    let mut recs = Vec::with_capacity(a.n);
    for i in 0..a.n {
        let mk = |rng: &mut Rng| {
            let p = (rng.next() as usize) % (genome.len() - rl);
            let mut s = genome[p..p + rl].to_vec();
            for b in &mut s {
                if rng.next().is_multiple_of(100) {
                    *b = b"ACGTN"[(rng.next() % 5) as usize];
                }
            }
            // illumina-like binned qualities
            let q: Vec<u8> = (0..rl)
                .map(|_| b"FFF:,#"[(rng.next() % 6) as usize])
                .collect();
            (s, q)
        };
        let (s, sq) = mk(&mut rng);
        let (x, xq) = mk(&mut rng);
        recs.push((s, sq, x, xq, format!("SRR0000001.{i} {i}/1")));
    }

    let dir = tempfile::tempdir()?;
    for fmt in [Format::Cbq, Format::Vbq] {
        if a.format
            .as_deref()
            .is_some_and(|f| f != fmt.extension().trim_start_matches('.'))
        {
            continue;
        }
        let path = dir.path().join(format!("b.{}", fmt.extension()));
        let (mut wt, mut rt) = (vec![], vec![]);
        for _ in 0..a.reps {
            let t = Instant::now();
            let mut w = BinseqWriterBuilder::new(fmt)
                .paired(true)
                .quality(true)
                .headers(true)
                .compression_level(a.level)
                .build(BufWriter::new(File::create(&path)?))?;
            for (s, sq, x, xq, h) in &recs {
                w.push(
                    SequencingRecordBuilder::default()
                        .s_seq(s)
                        .s_qual(sq)
                        .x_seq(x)
                        .x_qual(xq)
                        .s_header(h.as_bytes())
                        .x_header(h.as_bytes())
                        .build()?,
                )?;
            }
            w.finish()?;
            drop(w);
            wt.push(t.elapsed().as_secs_f64());

            let t = Instant::now();
            let reader = BinseqReader::new(&path)?;
            reader.process_parallel(Touch::default(), a.threads)?;
            rt.push(t.elapsed().as_secs_f64());
        }
        let min = |v: &[f64]| v.iter().copied().fold(f64::MAX, f64::min);
        let size = std::fs::metadata(&path)?.len();
        println!(
            "{:?}: size={size} write_best={:.3}s read_best={:.3}s (n={}, reps={})",
            fmt,
            min(&wt),
            min(&rt),
            a.n,
            a.reps
        );
    }
    Ok(())
}
