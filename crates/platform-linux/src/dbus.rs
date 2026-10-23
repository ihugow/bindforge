//! Inventory only; enumeration stays outside the compositor's message handler.
use remap_core::DeviceEnumerator;
use remap_ipc::dbus::{encode_devices, WireDevices};
use crate::LinuxDeviceEnumerator;

pub struct DevicesInterface;
#[zbus::interface(name = "org.bindforge.Daemon.Devices1")]
impl DevicesInterface {
    #[zbus(out_args("protocol_version", "batteries"))]
    async fn get_device_batteries(&self) -> zbus::fdo::Result<(u32, Vec<(String, i32, String)>)> {
        blocking::unblock(|| LinuxDeviceEnumerator.enumerate().map(|devices| (1, crate::battery::device_batteries(&devices))))
            .await.map_err(zbus::fdo::Error::Failed)
    }

    #[zbus(out_args("protocol_version", "devices"))]
    async fn get_devices(&self) -> zbus::fdo::Result<WireDevices> {
        blocking::unblock(|| LinuxDeviceEnumerator.enumerate().and_then(encode_devices))
            .await.map_err(zbus::fdo::Error::Failed)
    }
}
