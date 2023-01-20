use owowon::device::{PID, VID};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use windows::{
    Devices::{
        Enumeration::{DeviceInformation, DeviceInformationUpdate, DeviceWatcher},
        Usb::UsbDevice,
    },
    Foundation::{EventRegistrationToken, TypedEventHandler},
};

type DeviceList = Arc<RwLock<HashMap<String, DeviceInformation>>>;

pub struct DeviceSelector {
    list: DeviceList,
    watcher: DeviceWatcher,

    added_token: EventRegistrationToken,
    updated_token: EventRegistrationToken,
    removed_token: EventRegistrationToken,
}

impl DeviceSelector {
    pub fn list(&self) -> &DeviceList {
        &self.list
    }

    pub fn new(
        update_ui: impl Fn() + Send + Clone + 'static,
    ) -> Result<Self, windows::core::Error> {
        let list: DeviceList = Arc::new(RwLock::new(HashMap::new()));

        let selector = UsbDevice::GetDeviceSelectorVidPidOnly(VID, PID)?;
        let watcher = DeviceInformation::CreateWatcherAqsFilter(&selector)?;

        let list_added = list.clone();
        let update_ui_clone = update_ui.clone();
        let added_token = watcher.Added(&TypedEventHandler::new(move |a, b| {
            Self::added(&list_added, a, b)?;
            update_ui_clone();
            Ok(())
        }))?;

        let list_updated = list.clone();
        let update_ui_clone = update_ui.clone();
        let updated_token = watcher.Updated(&TypedEventHandler::new(move |a, b| {
            Self::updated(&list_updated, a, b)?;
            update_ui_clone();
            Ok(())
        }))?;

        let list_removed = list.clone();
        let update_ui_clone = update_ui;
        let removed_token = watcher.Removed(&TypedEventHandler::new(move |a, b| {
            Self::removed(&list_removed, a, b)?;
            update_ui_clone();
            Ok(())
        }))?;

        watcher.Start()?;

        Ok(Self {
            list,
            watcher,
            added_token,
            updated_token,
            removed_token,
        })
    }

    fn added(
        list: &DeviceList,
        _watcher: &Option<DeviceWatcher>,
        info: &Option<DeviceInformation>,
    ) -> Result<(), windows::core::Error> {
        let info = if let Some(info) = info {
            info
        } else {
            return Ok(());
        };

        let id = info.Id()?.to_string();
        list.blocking_write().insert(id, info.clone());

        Ok(())
    }

    fn updated(
        list: &DeviceList,
        _watcher: &Option<DeviceWatcher>,
        info_update: &Option<DeviceInformationUpdate>,
    ) -> Result<(), windows::core::Error> {
        let info_update = if let Some(info_update) = info_update {
            info_update
        } else {
            return Ok(());
        };

        let id = info_update.Id()?.to_string();

        if let Some(info) = list.blocking_write().get_mut(&id) {
            info.Update(info_update)?;
        }

        Ok(())
    }

    fn removed(
        list: &DeviceList,
        _watcher: &Option<DeviceWatcher>,
        info_update: &Option<DeviceInformationUpdate>,
    ) -> Result<(), windows::core::Error> {
        let info_update = if let Some(info_update) = info_update {
            info_update
        } else {
            return Ok(());
        };

        let id = info_update.Id()?.to_string();
        list.blocking_write().remove(&id);

        Ok(())
    }
}

impl Drop for DeviceSelector {
    fn drop(&mut self) {
        let _ = self.watcher.Stop();
        let _ = self.watcher.RemoveAdded(self.added_token);
        let _ = self.watcher.RemoveUpdated(self.updated_token);
        let _ = self.watcher.RemoveRemoved(self.removed_token);
    }
}
