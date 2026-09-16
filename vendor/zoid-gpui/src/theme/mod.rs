//! GPUI-facing theme module: conversion from [`ThemeTokens`] to
//! [`gpui_kit::component::ThemeColor`] for runtime palette selection.
//!
//! [`ThemeTokens`]: zoid_core::schema::ThemeTokens

pub mod conversion;

pub use conversion::theme_tokens_to_theme_color;
