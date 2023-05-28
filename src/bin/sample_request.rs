use arrow::array::StringArray;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use clap::Parser;
use itertools::Itertools;
use parquet::arrow::arrow_writer::ArrowWriter;
use parquet::file::properties::WriterProperties;
use resas_downloader::{client, schema};
use std::fs::File;
use std::sync::Arc;
use std::{str, thread, time::Duration};

const RESAS_PATH_PREFECTURE: &str = "api/v1/prefectures";
const RESAS_PATH_CITY: &str = "api/v1/cities";
const INTERVAL_MILLIS: u64 = 200;

#[derive(Parser)]
struct Args {
    token: String,
    output_path: String,
}
pub fn main() {
    let args = Args::parse();
    let (token, output_path) = (&args.token, &args.output_path);
    let client = client::Client::new(String::from(token.as_str()), client::RetryPolicy::default());
    let prefectures = match client.get::<schema::Prefecture>(RESAS_PATH_PREFECTURE, None, true) {
        Ok(p) => p.result,
        Err(e) => panic!("Failed to get request: {}", e),
    };

    let rows_iter = prefectures
        .iter()
        .flat_map(|p| {
            thread::sleep(Duration::from_millis(INTERVAL_MILLIS));
            let cities = client
                .get::<schema::City>(
                    RESAS_PATH_CITY,
                    Some(&format!("prefCode={}", p.pref_code)),
                    true,
                )
                .expect("Failed to get request")
                .result;
            println!("Fetched prefecture: {}", p.pref_name);
            cities.into_iter().map(|c| {
                vec![
                    c.pref_code.to_string(),
                    p.pref_name.clone(),
                    c.city_code,
                    c.city_name,
                    c.big_city_flag,
                ]
            })
        })
        .collect_vec();

    let n_columns = rows_iter
        .get(0)
        .expect("Not found data for city results")
        .len();

    //Transpose rows to columner.
    let columns_iter = (0..n_columns).map(|i| rows_iter.iter().map(move |j| j[i].clone()));

    let columns = columns_iter
        .map(|column| Arc::new(StringArray::from(column.collect_vec())) as arrow::array::ArrayRef)
        .collect_vec();

    let batch_cities = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("prefecture_code", DataType::Utf8, false),
            Field::new("prefecture_name", DataType::Utf8, false),
            Field::new("city_code", DataType::Utf8, false),
            Field::new("city_name", DataType::Utf8, false),
            Field::new("big_city_flag_array", DataType::Utf8, false),
        ])),
        columns,
    )
    .expect("Failed to genearte RecordBatch");

    let file =
        File::create(output_path).expect(&format!("Failed to create file at {}", output_path));
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(file, batch_cities.schema(), Some(props))
        .expect("Failed to create writer!");
    writer
        .write(&batch_cities)
        .expect("Failed to write RecordBatch");
    writer.close().expect("Failed to close writer");
    println!("Saved to {}", output_path);
}
