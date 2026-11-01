//! Image settings and real read-tool output before model-message conversion.
use super::super::*;
use maestro_app::SettingsManager;

/// An omitted image-blocking preference leaves images enabled.
#[test]
fn image_blocking_defaults_to_false() -> Result<(), Error> {
    let settings = SettingsManager::in_memory(serde_json::from_value(json!({}))?);
    assert!(!settings.get_block_images());
    Ok(())
}

/// Explicit image blocking reaches the typed settings reader.
#[test]
fn configured_image_blocking_reads_true() -> Result<(), Error> {
    let settings = SettingsManager::in_memory(serde_json::from_value(
        json!({"images":{"blockImages":true}}),
    )?);
    assert!(settings.get_block_images());
    Ok(())
}

/// The setter publishes both transitions immediately.
#[test]
fn image_blocking_setter_retains_each_selection() -> Result<(), Error> {
    let mut settings = SettingsManager::in_memory(serde_json::from_value(json!({}))?);
    settings.set_block_images(true);
    assert!(settings.get_block_images());
    settings.set_block_images(false);
    assert!(!settings.get_block_images());
    Ok(())
}

/// Blocking and resizing remain independently readable from one image preference object.
#[test]
fn image_blocking_coexists_with_automatic_resizing() -> Result<(), Error> {
    let settings = SettingsManager::in_memory(serde_json::from_value(
        json!({"images":{"blockImages":true,"autoResize":true}}),
    )?);
    assert!(settings.get_block_images());
    assert!(settings.get_image_auto_resize());
    Ok(())
}

/// Execute the SDK-created read tool against a relative path in its real working directory.
async fn read(sample: Sample, path: &str, bytes: &[u8]) -> Result<Value, Error> {
    use std::{cell::RefCell, rc::Rc};
    std::fs::write(sample.root.path().join(path), bytes)?;
    let mut options = sample.options()?;
    options.settings_manager = Some(Rc::new(RefCell::new(SettingsManager::in_memory(
        serde_json::from_value(json!({"images":{"blockImages":true}}))?,
    ))));
    let created = maestro_app::create_agent_session(options).await?;
    let execute = {
        let state = created.session.state().read().unwrap();
        let tools = state.tools().read().unwrap();
        let read = tools
            .iter()
            .find(|tool| tool.read().unwrap().definition.name == "read")
            .unwrap();
        read.read().unwrap().execute.clone()
    };
    let result = execute("read-image".into(), json!({"path":path}), None, None).await?;
    created.session.dispose();
    Ok(serde_json::to_value(result)?)
}

/// Blocking is applied by message conversion, never by the native image read itself.
#[test]
fn read_tools_keep_images_when_model_image_blocking_is_enabled() -> Result<(), Error> {
    use base64::Engine as _;
    support::execute(|sample| async move {
        let png = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==")?;
        let result = read(sample, "image.png", &png).await?;
        assert!(
            result["content"]
                .as_array()
                .unwrap()
                .iter()
                .any(|block| block["type"] == "image")
        );
        Ok(Value::Null)
    })?;
    Ok(())
}

/// Ordinary text still comes from the same SDK-created read tool.
#[test]
fn read_tools_keep_text_when_model_image_blocking_is_enabled() -> Result<(), Error> {
    support::execute(|sample| async move {
        let result = read(sample, "hello.txt", b"Hello, world!").await?;
        assert_eq!(result["content"].as_array().unwrap().len(), 1);
        assert_eq!(result["content"][0]["type"], "text");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Hello, world!")
        );
        Ok(Value::Null)
    })?;
    Ok(())
}
