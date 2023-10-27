use anyhow::Error;
use clap::{Args, Parser};
use serde::{Deserialize, Serialize};
use spin_app::MetadataKey;
use spin_core::async_trait;
use spin_trigger::{
    cli::TriggerExecutorCommand, EitherInstance, TriggerAppEngine, TriggerExecutor,
};

use spin::mqtt_trigger_sdk::{mqtt_types::Payload, outbound_mqtt::Host};

wasmtime::component::bindgen!({
    path: ".",
    world: "spin-mqtt",
    async: true,
});

pub(crate) type RuntimeData = ();
pub(crate) type _Store = spin_core::Store<RuntimeData>;
type Command = TriggerExecutorCommand<MqttTrigger>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let trigger = Command::parse();
    trigger.run().await
}

#[derive(Args)]
pub struct CliArgs {
    #[clap(long)]
    pub test: bool,
}

// The trigger structure with all values processed and ready
struct MqttTrigger {
    engine: TriggerAppEngine<Self>,
    address: String,
    qos: u8,
    component_configs: Vec<(String, u8, String)>,
}

// Application settings (raw serialization format)
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TriggerMetadata {
    r#type: String,
    address: String,
    qos: u8,
}

// Per-component settings (raw serialization format)
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MqttTriggerConfig {
    component: String,
    topic: String,
    qos: u8,
}

const TRIGGER_METADATA_KEY: MetadataKey<TriggerMetadata> = MetadataKey::new("trigger");

#[async_trait]
impl TriggerExecutor for MqttTrigger {
    const TRIGGER_TYPE: &'static str = "mqtt";
    type RuntimeData = RuntimeData;
    type TriggerConfig = MqttTriggerConfig;
    type RunConfig = CliArgs;

    async fn new(engine: spin_trigger::TriggerAppEngine<Self>) -> anyhow::Result<Self> {
        let address = engine.app().require_metadata(TRIGGER_METADATA_KEY)?.address;
        let qos = engine.app().require_metadata(TRIGGER_METADATA_KEY)?.qos;

        let component_configs = engine
            .trigger_configs()
            .map(|(_, config)| (config.component.clone(), config.qos, config.topic.clone()))
            .collect();

        Ok(Self {
            engine,
            address,
            qos,
            component_configs,
        })
    }

    async fn run(self, _config: Self::RunConfig) -> anyhow::Result<()> {
        // This trigger spawns threads, which Ctrl+C does not kill.  So
        // for this case we need to detect Ctrl+C and shut those threads
        // down. For simplicity, we do this by terminating the process.
        println!(
            "Executing trigger with address {}, qos {}...",
            &self.address, &self.qos
        );

        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            std::process::exit(0);
        });

        tokio_scoped::scope(|scope| {
            for (component_id, mqtt_qos, mqtt_topic) in &self.component_configs {
                println!(
                    "Executing component {}, topic {}, qos {}...",
                    &component_id, &mqtt_topic, &mqtt_qos
                );

                scope.spawn(async {
                    self.handle_mqtt_event(component_id, mqtt_qos, mqtt_topic)
                        .await
                        .unwrap();
                });
            }
        });

        Ok(())
    }
}

impl MqttTrigger {
    async fn handle_mqtt_event(
        &self,
        component_id: &str,
        mqtt_qos: &u8,
        mqtt_topic: &str,
    ) -> anyhow::Result<()> {
        println!("Executing component handler for {component_id}, {mqtt_qos}, {mqtt_topic}...");

        // // Load the wasm component
        let (instance, mut store) = self.engine.prepare_instance(component_id).await?;
        let EitherInstance::Component(instance) = instance else {
            unreachable!()
        };

        // SpinMqtt is auto generated by bindgen as per WIT files referenced above.
        let instance = SpinMqtt::new(&mut store, &instance)?;

        // TODO: return this instead of OK(())
        let _result = instance
            .spin_mqtt_trigger_sdk_inbound_mqtt()
            .call_handle_message(store, &"dummy mqtt data".to_string().as_bytes().to_vec())
            .await;
        Ok(())
    }
}

#[async_trait]
impl Host for SpinMqtt {
    async fn publish(
        &mut self,
        topic: String,
        payload: Payload,
    ) -> Result<std::result::Result<(), spin::mqtt_trigger_sdk::mqtt_types::Error>, Error> {
        println!(
            "Publishing on behalf of wasm component: {}, address {}, Qos: {}, Topic: {}...",
            String::from_utf8_lossy(&payload),
            &"self.address",
            &"self.qos",
            topic
        );

        // TODO: implement MQTT publish here
        Ok(Ok(()))
    }
}

#[async_trait]
impl Host for MqttTrigger {
    async fn publish(
        &mut self,
        topic: String,
        payload: Payload,
    ) -> Result<std::result::Result<(), spin::mqtt_trigger_sdk::mqtt_types::Error>, Error> {
        println!(
            "Publishing on behalf of wasm component: {}, address {}, Qos: {}, Topic: {}...",
            String::from_utf8_lossy(&payload),
            &"self.address",
            &"self.qos",
            topic
        );

        // TODO: implement MQTT publish here
        Ok(Ok(()))
    }
}