This goal of this repository. is to allow users to reproduce results of [this](https://blog.peerdb.io/benchmarking-postgres-replication-peerdb-vs-airbyte) blog post that compared the performance of [PeerDB](https://peerdb.io) and [Airbyte](https://airbyte.com). All information regarding PeerDB and Airbyte internals is accurate as of October 2023, and might change in the future.

## Benchmark Setup
The major components of the database were all provisioned in the `us-east-2` AWS region.

### Source Postgres
The source Postgres instance was provisioned on AWS RDS and was of type `db.r7g.4xlarge`. Logical Replication had to enabled to support CDC mirrors for both PeerDB and Airbyte.

### Target Snowflake
The compute warehouse for these runs was set to `LARGE`. No other notable changes were made to the cluster.

### Migration Platform
Both PeerDB and Airbyte were setup using their Docker Compose offerings available on their respective repositories. We set them up on the same `c7g.metal` AWS EC2 instance, and at no point was PeerDB and Airbyte running simultaneously. 

### Data Generation
We chose to work with the schema Airbyte used in their [benchmarking](https://airbyte.com/blog/postgres-replication-performance-benchmark-airbyte-vs-fivetran) against Fivetran. The only change we made was making column `f0` the primary key, which was also a [generated column](https://www.postgresql.org/docs/current/ddl-generated-columns.html).

```
CREATE TABLE IF NOT EXISTS firenibble
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
);
```

Aside from the generated primary key, all columns had randomly generated data loaded into them. All `VARCHAR` column were populated with a 32-character random string. The data generation and loading was done by a Rust-based tool located in this this repository, which can be configured by setting the appropriate parameters in `src/main.rs` and built by invoking `cargo build --release` at the root of this repository.

## Benchmark Info
A table with the following schema was generated on the source Postgres instance. The table was then populated with 6 billion randomly generated rows. PeerDB and Airbyte were then asked to sync this table to different Snowflake databases on the same cluster.

### PeerDB
PeerDB currently does not a web interface, and all connectors and mirrors are setup via our Postgres-compatible SQL interface. We set up the connectors and mirrors as per [this](https://docs.peerdb.io/usecases/Real-time%20CDC/postgres-to-snowflake) guide. As we set `do_initial_copy`, a parallelized initial snapshot is executed. 

This workflow has three main steps. We first determine the number of rows in each table we need to snapshot, and we use this information to divide the table up into CTID-based partitions. We then spin up the requested number of parallel workers and divide the partitions among them, ensuring that each worker processes its partitions independently. Transport of the rows to a Snowflake stage is done via Avro files, with the number of rows per Avro file being configurable.  After all partitions have been copied over, a `COPY INTO` command is executed on the Snowflake side, populating the destination tables with their corresponding rows. Basic metrics for the run are logged in the `peerdb_stats.qrep_runs` table, from which we extract our timing information.

Apart from adjusting `snapshot_max_parallel_workers` to increase parallelism of the initial load, we also tuned `snapshot_num_rows_per_partition` to increase the size of each partition. This had the effect of reducing the number of network round-trips for each parallel worker which made the transport of all the rows faster. Snowflake also benefits from having to process less files of a larger size, although this principle shouldn't be taken too far.

We executed runs with 1, 8, 16, 32 and 48 parallel threads. This provides a range of performance numbers, ranging from the lightweight single-thread case to a massive 48-thread parallelism case that takes advantage of beefier hardware and transport. 

### Airbyte
Airbyte has a web interface, which allows us to conveniently create connectors and monitor mirror status. We created a Postgres source, Snowflake destination and a connection between them according to [this](https://airbyte.com/tutorials/postgresql-database-to-snowflake) guide, selecting only the single table we want to sync. We use a Snowflake internal stage, just like with PeerDB.

From what we could determine, Airbyte uses a CTID-based `SELECT` statement to iterate over the table. The transport of the rows is done via compressed CSV files, with a fixed number of rows in each file. After the rows are transported, a `COPY INTO` query is issued to move records into a staging table followed by queries to insert rows into the final table from the staging table.

There were no options exposed in the web interface that pertained to mirror performance. As a result, only one Airbyte run was executed as part of testing.


