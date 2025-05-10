/*
 * AWS Utilities Module
 * 
 * This module provides utility functions for interacting with AWS services:
 * - Loading and configuring the AWS SDK
 * - Listing available S3 buckets
 * - Determining the region for a specific S3 bucket
 * 
 * These functions abstract away the details of AWS API interactions and provide
 * a simpler interface for the main application to use.
 */

use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_config::{Region, SdkConfig};
use aws_sdk_s3::config::StalledStreamProtectionConfig;
use aws_sdk_s3::Client;

/// Loads and configures the AWS SDK with appropriate settings
///
/// # Parameters
/// * `region` - Optional AWS region to use; if None, uses the default provider chain or falls back to us-east-1
///
/// # Returns
/// * `SdkConfig` - Configured AWS SDK configuration
///
/// # Details
/// - Configures the AWS SDK using environment variables and credentials files
/// - Sets the specified region or uses the default provider chain
/// - Disables stalled stream protection to resolve issues with large S3 file uploads
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

/// Lists all S3 buckets available to the authenticated user
///
/// # Parameters
/// * `client` - AWS S3 client instance
///
/// # Returns
/// * `Result<Vec<String>, Error>` - List of bucket names or an error
///
/// # Details
/// - Makes an API call to S3 to list all buckets
/// - Extracts the bucket names from the response
/// - Returns the names as a vector of strings
pub async fn list_buckets(client: &Client) -> Result<Vec<String>> {
    let resp = client.list_buckets().send().await?;
    let buckets = resp.buckets();

    let bucket_names: Vec<String> = buckets
        .iter()
        .map(|bucket| bucket.name().unwrap_or_default().to_string())
        .collect();

    Ok(bucket_names)
}

/// Determines the AWS region for a specific S3 bucket
///
/// # Parameters
/// * `client` - AWS S3 client instance
/// * `bucket_name` - Name of the S3 bucket to check
///
/// # Returns
/// * `Result<Region, Error>` - The bucket's region or an error
///
/// # Details
/// - Makes an API call to get the bucket's location constraint
/// - Handles the special case where an empty location constraint means us-east-1
/// - Returns a Region object that can be used to configure region-specific clients
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