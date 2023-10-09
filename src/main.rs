use log::{error, info};
use postgres::binary_copy::BinaryCopyInWriter;
use postgres::config::SslMode;
use postgres::types::{ToSql, Type};
use postgres::{Client, Config, NoTls};
use std::thread;
use std::time::Instant;

use rand::distributions::{Alphanumeric, DistString};
use rand::Rng;
use time::Date;

// calculated in order to get a table of a certain size.
const RANDOM_STRING_LENGTH: usize = 32;
// total records generated is TOTAL_BATCHES * RECORDS_PER_BATCH.
// TOTAL_BATCHES should be a multiple of PARALLELISM.
const TOTAL_BATCHES: usize = 40;
const RECORDS_PER_BATCH: usize = 250_000;
// the number of batches to be generated is divided evenly across the parallel threads.
const PARALLELISM: usize = 16;

struct Record {
    f1: i64,
    f2: i64,
    f3: i32,
    f4: f64,
    f5: f64,
    f6: f64,
    f7: f64,
    f8: String,
    f9: String,
    f10: Date,
    f11: Date,
    f12: Date,
    f13: String,
    f14: String,
    f15: String,
}

// set the parameters of your database here.
fn initialize_client() -> Client {
    let mut config = Config::new();
    config.host("[HOST HERE]");
    config.port(0);
    config.user("[USER HERE]");
    config.password(r"[PASSWORD HERE]");
    config.dbname("[DATABASE HERE]");
    config.ssl_mode(SslMode::Disable);

    return config.connect(NoTls).unwrap();
}

fn setup_table() {
    let mut client = initialize_client();

    // not dropping the table explicitly
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS firenibble
    (    
      f0 BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
      f1 BIGINT,
      f2 BIGINT,
      f3 INTEGER,
      f4 DOUBLE PRECISION,
      f5 DOUBLE PRECISION,
      f6 DOUBLE PRECISION,
      f7 DOUBLE PRECISION,
      f8 VARCHAR COLLATE pg_catalog.\"default\",
      f9 VARCHAR COLLATE pg_catalog.\"default\",
      f10 DATE,
      f11 DATE,
      f12 DATE,
      f13 VARCHAR COLLATE pg_catalog.\"default\",
      f14 VARCHAR COLLATE pg_catalog.\"default\",
      f15 VARCHAR COLLATE pg_catalog.\"default\"
    )",
            &[],
        )
        .unwrap();

    info!("finished creating target table")
}

fn processor(process_id: usize) {
    let mut client = initialize_client();
    let mut rng = rand::thread_rng();

    for batch_id in 0..(TOTAL_BATCHES / PARALLELISM) {
        let mut records = Vec::with_capacity(RECORDS_PER_BATCH as usize);
        for _ in 0..RECORDS_PER_BATCH {
            records.push(Record {
                f1: rng.gen(),
                f2: rng.gen(),
                f3: rng.gen(),
                f4: rng.gen(),
                f5: rng.gen(),
                f6: rng.gen(),
                f7: rng.gen(),
                f8: Alphanumeric.sample_string(&mut rng, RANDOM_STRING_LENGTH),
                f9: Alphanumeric.sample_string(&mut rng, RANDOM_STRING_LENGTH),
                f10: Date::from_ordinal_date(rng.gen_range(1..9999), rng.gen_range(1..365)).unwrap(),
                f11: Date::from_ordinal_date(rng.gen_range(1..9999), rng.gen_range(1..365)).unwrap(),
                f12: Date::from_ordinal_date(rng.gen_range(1..9999), rng.gen_range(1..365)).unwrap(),
                f13: Alphanumeric.sample_string(&mut rng, RANDOM_STRING_LENGTH),
                f14: Alphanumeric.sample_string(&mut rng, RANDOM_STRING_LENGTH),
                f15: Alphanumeric.sample_string(&mut rng, RANDOM_STRING_LENGTH),
            })
        }

        let copy_sink = client
            .copy_in("COPY firenibble(f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11,f12,f13,f14,f15) FROM STDIN BINARY")
            .unwrap();
        let mut copy_writer = BinaryCopyInWriter::new(
            copy_sink,
            &[
                Type::INT8,
                Type::INT8,
                Type::INT4,
                Type::FLOAT8,
                Type::FLOAT8,
                Type::FLOAT8,
                Type::FLOAT8,
                Type::VARCHAR,
                Type::VARCHAR,
                Type::DATE,
                Type::DATE,
                Type::DATE,
                Type::VARCHAR,
                Type::VARCHAR,
                Type::VARCHAR,
            ],
        );

        for record in &records {
            let row: Vec<&'_ (dyn ToSql + Sync)> = vec![
                &record.f1,
                &record.f2,
                &record.f3,
                &record.f4,
                &record.f5,
                &record.f6,
                &record.f7,
                &record.f8,
                &record.f9,
                &record.f10,
                &record.f11,
                &record.f12,
                &record.f13,
                &record.f14,
                &record.f15,
            ];
            copy_writer.write(row.as_slice()).unwrap();
        }

        let row_count = copy_writer.finish().unwrap();
        if row_count as usize != RECORDS_PER_BATCH {
            error!(
                "[Process/Batch #{}/#{}] Failed to write some rows: {}/{} rows written",
                process_id, batch_id, row_count, RECORDS_PER_BATCH
            );
        } else {
            info!(
                "[Process/Batch #{}/#{}] {} rows written",
                process_id, batch_id, row_count
            );
        }
    }
}

fn main() {
    env_logger::init();

    setup_table();

    let start = Instant::now();
    thread::scope(|s| {
        for process_id in 0..PARALLELISM {
            s.spawn(move || processor(process_id));
        }
    });
    let elapsed = start.elapsed();
    info!(
        "Finished inserting {} rows in {:.2?} seconds",
        TOTAL_BATCHES * RECORDS_PER_BATCH,
        elapsed
    );
    info!(
        "Throughput: {:.2}",
        (TOTAL_BATCHES * RECORDS_PER_BATCH) as f64 / elapsed.as_secs_f64()
    );

    ()
}
