use std::sync::Arc;

use esp_idf_svc::{
    bt::{
        ble::gap::{AdvConfiguration, BleGapEvent, EspBleGap},
        Ble, BtDriver, BtStatus,
    },
    hal::modem::BluetoothModem,
    nvs::EspDefaultNvsPartition,
    sys::{EspError, ESP_BLE_ADV_FLAG_BREDR_NOT_SPT, ESP_BLE_ADV_FLAG_GEN_DISC},
};
use log::{info, warn};

pub type BleGap = Arc<EspBleGap<'static, Ble, Arc<BtDriver<'static, Ble>>>>;

const DEVICE_NAME: &str = "JPod";

/// Starts advertising as `DEVICE_NAME`. Keep the returned handle alive or BLE shuts down.
pub fn run(modem: BluetoothModem<'static>, nvs: EspDefaultNvsPartition) -> Result<BleGap, EspError> {
    let bt = Arc::new(BtDriver::new(modem, Some(nvs))?);
    let gap: BleGap = Arc::new(EspBleGap::new(bt)?);

    // Weak ref so the callback doesn't keep the GAP alive forever.
    let weak = Arc::downgrade(&gap);
    gap.subscribe(move |event| match event {
        BleGapEvent::AdvertisingConfigured(status) => {
            if status != BtStatus::Success {
                warn!("ble: adv config failed: {status:?}");
            } else if let Some(gap) = weak.upgrade() {
                if let Err(e) = gap.start_advertising() {
                    warn!("ble: start_advertising failed: {e:?}");
                }
            }
        }
        BleGapEvent::AdvertisingStarted(status) => {
            info!("ble: advertising as \"{DEVICE_NAME}\": {status:?}");
        }
        _ => {}
    })?;

    gap.set_device_name(DEVICE_NAME)?;
    // Advertising actually starts in the AdvertisingConfigured callback above.
    gap.set_adv_conf(&AdvConfiguration {
        include_name: true,
        include_txpower: true,
        flag: (ESP_BLE_ADV_FLAG_GEN_DISC | ESP_BLE_ADV_FLAG_BREDR_NOT_SPT) as u8,
        ..Default::default()
    })?;

    Ok(gap)
}
