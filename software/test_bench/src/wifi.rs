use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::WifiModem,
    nvs::EspDefaultNvsPartition,
    sys::EspError,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use log::info;

// Baked in at build time: `WIFI_SSID=... WIFI_PASS=... cargo run`. Unset = scan only.
const SSID: Option<&str> = option_env!("WIFI_SSID");
const PASS: Option<&str> = option_env!("WIFI_PASS");

pub fn run(
    modem: WifiModem<'static>,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
) -> Result<(), EspError> {
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    let client = match SSID {
        Some(ssid) => ClientConfiguration {
            ssid: ssid.try_into().expect("SSID too long"),
            password: PASS.unwrap_or("").try_into().expect("password too long"),
            auth_method: if PASS.is_some() {
                AuthMethod::WPA2Personal
            } else {
                AuthMethod::None
            },
            ..Default::default()
        },
        None => ClientConfiguration::default(),
    };
    wifi.set_configuration(&Configuration::Client(client))?;
    wifi.start()?;

    let aps = wifi.scan()?;
    info!("wifi: scan found {} APs", aps.len());
    for ap in &aps {
        info!("wifi:   {} (ch {}, {} dBm)", ap.ssid, ap.channel, ap.signal_strength);
    }

    if SSID.is_none() {
        info!("wifi: WIFI_SSID not set, skipping connect");
        return Ok(());
    }

    wifi.connect()?;
    wifi.wait_netif_up()?;
    info!("wifi: connected, {:?}", wifi.wifi().sta_netif().get_ip_info()?);

    Ok(())
}
