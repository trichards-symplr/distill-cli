use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_config::{Region, SdkConfig};
use aws_sdk_s3::config::StalledStreamProtectionConfig;
use aws_sdk_s3::Client;

/// Load the user's AWS config, default region to us-east-1 if none is provided or can be found
pub async fn load_config(region: Option<Region>) -> SdkConfig {
    let mut config = aws_config::from_env();
    match region {
        Some(region) => config = config.region(region),
        None => {
            config = config.region(RegionProviderChain::default_provider().or_else("us-east-1"))
        }
    }

    // Resolves issues with uploading large S3 files
    // See https://github.com/awslabs/aws-sdk-rust/issues/1146
    config = config
        .stalled_stream_protection(
            StalledStreamProtectionConfig::disabled()
        );

    config.load().await
}

/// List all S3 buckets available to the user
pub async fn list_buckets(client: &Client) -> Result<Vec<String>> {
    let resp = client.list_buckets().send().await?;
    let buckets = resp.buckets();

    let bucket_names: Vec<String> = buckets
        .iter()
        .map(|bucket| bucket.name().unwrap_or_default().to_string())
        .collect();

    Ok(bucket_names)
}

/// Get the region for a specific S3 bucket
pub async fn bucket_region(client: &Client, bucket_name: &str) -> Result<Region> {
    let resp = client
        .get_bucket_location()
        .bucket(bucket_name)
        .send()
        .await?;

    let location_constraint = resp
        .location_constraint()
        .context("Bucket has no location_constraint")?;

    if location_constraint.as_str() == "" {
        Ok(Region::new("us-east-1"))
    } else {
        Ok(Region::new(location_constraint.as_str().to_owned()))
    }
}