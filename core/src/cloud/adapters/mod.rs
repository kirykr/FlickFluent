//! One adapter per service. Vendor formats stay in here.

pub mod azure;
pub mod gemini;

use super::adapter::CloudAdapter;
use super::services::Service;

/// The adapter for a service in the data file, or `None` for an id nothing implements.
pub fn adapter_for(service: &Service) -> Option<Box<dyn CloudAdapter>> {
    match service.id.as_str() {
        "google-gemini" => Some(Box::new(gemini::GeminiAdapter::new(service.clone()))),
        "azure-ai" => Some(Box::new(azure::AzureAdapter::new(service.clone()))),
        _ => None,
    }
}
