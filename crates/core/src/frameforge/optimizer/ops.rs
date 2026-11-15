//! The system surface the optimiser is allowed to touch, as one trait.
//!
//! Every write the optimiser can make to Windows goes through [`SystemOps`]:
//! power schemes (documented power-management API) and per-user registry
//! values under `HKEY_CURRENT_USER`. There is deliberately no method for
//! services, boot configuration, machine-wide policy, or anything on the
//! "never" list, so an action cannot reach them even by mistake.
//!
//! [`MockOps`] is the same surface in memory; tests and the non-Windows
//! build use it, so no test changes the machine it runs on.

use crate::frameforge::hardware::PowerPlan;
use crate::frameforge::processes::ProcessOps;
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};

pub trait SystemOps: Send + Sync {
    fn power_plans(&self) -> Vec<PowerPlan>;
    fn set_active_power_plan(&self, guid: &str) -> anyhow::Result<()>;
    /// Copy a plan under a new name; returns the copy's id.
    fn duplicate_power_plan(&self, source: &str, name: &str) -> anyhow::Result<String>;
    fn delete_power_plan(&self, guid: &str) -> anyhow::Result<()>;
    /// Plugged-in (AC) value of one power setting.
    fn read_power_value(&self, plan: &str, subgroup: &str, setting: &str) -> anyhow::Result<u32>;
    fn write_power_value(&self, plan: &str, subgroup: &str, setting: &str, value: u32) -> anyhow::Result<()>;
    fn on_battery(&self) -> Option<bool>;

    fn user_u32(&self, path: &str, name: &str) -> Option<u32>;
    fn set_user_u32(&self, path: &str, name: &str, value: u32) -> anyhow::Result<()>;
    fn user_string(&self, path: &str, name: &str) -> Option<String>;
    fn set_user_string(&self, path: &str, name: &str, value: &str) -> anyhow::Result<()>;
    fn delete_user_value(&self, path: &str, name: &str) -> anyhow::Result<()>;
}

/// The real machine.
pub struct WindowsOps;

#[cfg(windows)]
impl SystemOps for WindowsOps {
    fn power_plans(&self) -> Vec<PowerPlan> {
        crate::frameforge::winsys::power_schemes().into_iter().map(|(guid, name, active)| PowerPlan { guid, name, active }).collect()
    }
    fn set_active_power_plan(&self, guid: &str) -> anyhow::Result<()> {
        crate::frameforge::winsys::set_active_power_scheme(guid)
    }
    fn duplicate_power_plan(&self, source: &str, name: &str) -> anyhow::Result<String> {
        crate::frameforge::winsys::duplicate_power_scheme(source, name)
    }
    fn delete_power_plan(&self, guid: &str) -> anyhow::Result<()> {
        crate::frameforge::winsys::delete_power_scheme(guid)
    }
    fn read_power_value(&self, plan: &str, subgroup: &str, setting: &str) -> anyhow::Result<u32> {
        crate::frameforge::winsys::read_ac_value(plan, subgroup, setting)
    }
    fn write_power_value(&self, plan: &str, subgroup: &str, setting: &str, value: u32) -> anyhow::Result<()> {
        crate::frameforge::winsys::write_ac_value(plan, subgroup, setting, value)
    }
    fn on_battery(&self) -> Option<bool> {
        crate::frameforge::winsys::on_battery()
    }
    fn user_u32(&self, path: &str, name: &str) -> Option<u32> {
        crate::registry::read_u32(crate::registry::Hive::CurrentUser, path, name)
    }
    fn set_user_u32(&self, path: &str, name: &str, value: u32) -> anyhow::Result<()> {
        Ok(crate::registry::write_user_u32(path, name, value)?)
    }
    fn user_string(&self, path: &str, name: &str) -> Option<String> {
        crate::registry::read_string(crate::registry::Hive::CurrentUser, path, name)
    }
    fn set_user_string(&self, path: &str, name: &str, value: &str) -> anyhow::Result<()> {
        Ok(crate::registry::write_user_string(path, name, value)?)
    }
    fn delete_user_value(&self, path: &str, name: &str) -> anyhow::Result<()> {
        Ok(crate::registry::delete_user_value(path, name)?)
    }
}

#[cfg(not(windows))]
impl SystemOps for WindowsOps {
    fn power_plans(&self) -> Vec<PowerPlan> {
        Vec::new()
    }
    fn set_active_power_plan(&self, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("Power plans only exist on Windows")
    }
    fn duplicate_power_plan(&self, _: &str, _: &str) -> anyhow::Result<String> {
        anyhow::bail!("Power plans only exist on Windows")
    }
    fn delete_power_plan(&self, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("Power plans only exist on Windows")
    }
    fn read_power_value(&self, _: &str, _: &str, _: &str) -> anyhow::Result<u32> {
        anyhow::bail!("Power plans only exist on Windows")
    }
    fn write_power_value(&self, _: &str, _: &str, _: &str, _: u32) -> anyhow::Result<()> {
        anyhow::bail!("Power plans only exist on Windows")
    }
    fn on_battery(&self) -> Option<bool> {
        None
    }
    fn user_u32(&self, _: &str, _: &str) -> Option<u32> {
        None
    }
    fn set_user_u32(&self, _: &str, _: &str, _: u32) -> anyhow::Result<()> {
        anyhow::bail!("The registry only exists on Windows")
    }
    fn user_string(&self, _: &str, _: &str) -> Option<String> {
        None
    }
    fn set_user_string(&self, _: &str, _: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("The registry only exists on Windows")
    }
    fn delete_user_value(&self, _: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("The registry only exists on Windows")
    }
}

#[derive(Default)]
pub struct MockState {
    pub plans: Vec<PowerPlan>,
    pub power_values: BTreeMap<(String, String, String), u32>,
    pub u32s: BTreeMap<(String, String), u32>,
    pub strings: BTreeMap<(String, String), String>,
    pub on_battery: Option<bool>,
    /// Every successful write, in order, for assertions.
    pub log: Vec<String>,
    /// A write whose description contains this text fails.
    pub fail_on: Option<String>,
    /// Writes report success but change nothing (to test read-back verification).
    pub ignore_writes: bool,
    next_guid: u32,
}

/// In-memory [`SystemOps`].
#[derive(Default)]
pub struct MockOps(pub Mutex<MockState>);

impl MockOps {
    /// A desktop on the Balanced plan with High performance and Power saver available.
    pub fn typical() -> Self {
        let m = Self::default();
        {
            let mut s = m.0.lock();
            s.plans = vec![
                PowerPlan { guid: super::actions::PLAN_BALANCED.into(), name: "Balanced".into(), active: true },
                PowerPlan { guid: super::actions::PLAN_HIGH_PERFORMANCE.into(), name: "High performance".into(), active: false },
                PowerPlan { guid: super::actions::PLAN_POWER_SAVER.into(), name: "Power saver".into(), active: false },
            ];
            s.on_battery = Some(false);
        }
        m
    }

    fn write(&self, what: String, f: impl FnOnce(&mut MockState)) -> anyhow::Result<()> {
        let mut s = self.0.lock();
        if s.fail_on.as_deref().map(|needle| what.contains(needle)).unwrap_or(false) {
            anyhow::bail!("mock refused: {what}");
        }
        if !s.ignore_writes {
            f(&mut s);
        }
        s.log.push(what);
        Ok(())
    }
}

impl SystemOps for MockOps {
    fn power_plans(&self) -> Vec<PowerPlan> {
        self.0.lock().plans.clone()
    }
    fn set_active_power_plan(&self, guid: &str) -> anyhow::Result<()> {
        if !self.0.lock().plans.iter().any(|p| p.guid == guid) {
            anyhow::bail!("no such power plan {guid}");
        }
        self.write(format!("activate {guid}"), |s| s.plans.iter_mut().for_each(|p| p.active = p.guid == guid))
    }
    fn duplicate_power_plan(&self, source: &str, name: &str) -> anyhow::Result<String> {
        let guid = {
            let mut s = self.0.lock();
            if !s.plans.iter().any(|p| p.guid == source) {
                anyhow::bail!("no such power plan {source}");
            }
            s.next_guid += 1;
            format!("00000000-0000-0000-0000-{:012x}", s.next_guid)
        };
        let g = guid.clone();
        self.write(format!("duplicate {source} as {name}"), move |s| {
            let copied: Vec<_> = s.power_values.iter().filter(|((p, _, _), _)| p == source).map(|((_, a, b), v)| ((g.clone(), a.clone(), b.clone()), *v)).collect();
            s.power_values.extend(copied);
            s.plans.push(PowerPlan { guid: g, name: name.into(), active: false });
        })?;
        Ok(guid)
    }
    fn delete_power_plan(&self, guid: &str) -> anyhow::Result<()> {
        if self.0.lock().plans.iter().any(|p| p.guid == guid && p.active) {
            anyhow::bail!("the active power plan cannot be deleted");
        }
        self.write(format!("delete plan {guid}"), |s| s.plans.retain(|p| p.guid != guid))
    }
    fn read_power_value(&self, plan: &str, subgroup: &str, setting: &str) -> anyhow::Result<u32> {
        Ok(self.0.lock().power_values.get(&(plan.into(), subgroup.into(), setting.into())).copied().unwrap_or(0))
    }
    fn write_power_value(&self, plan: &str, subgroup: &str, setting: &str, value: u32) -> anyhow::Result<()> {
        self.write(format!("power {plan} {setting} = {value}"), |s| {
            s.power_values.insert((plan.into(), subgroup.into(), setting.into()), value);
        })
    }
    fn on_battery(&self) -> Option<bool> {
        self.0.lock().on_battery
    }
    fn user_u32(&self, path: &str, name: &str) -> Option<u32> {
        self.0.lock().u32s.get(&(path.into(), name.into())).copied()
    }
    fn set_user_u32(&self, path: &str, name: &str, value: u32) -> anyhow::Result<()> {
        self.write(format!("HKCU\\{path}\\{name} = {value}"), |s| {
            s.u32s.insert((path.into(), name.into()), value);
        })
    }
    fn user_string(&self, path: &str, name: &str) -> Option<String> {
        self.0.lock().strings.get(&(path.into(), name.into())).cloned()
    }
    fn set_user_string(&self, path: &str, name: &str, value: &str) -> anyhow::Result<()> {
        self.write(format!("HKCU\\{path}\\{name} = {value}"), |s| {
            s.strings.insert((path.into(), name.into()), value.into());
        })
    }
    fn delete_user_value(&self, path: &str, name: &str) -> anyhow::Result<()> {
        self.write(format!("delete HKCU\\{path}\\{name}"), |s| {
            s.u32s.remove(&(path.into(), name.into()));
            s.strings.remove(&(path.into(), name.into()));
        })
    }
}

/// In-memory [`ProcessOps`]: pid → priority class. A pid that is not in the map is "not running".
#[derive(Default)]
pub struct MockProcs {
    pub running: Mutex<HashMap<u32, String>>,
    pub closed: Mutex<Vec<u32>>,
    pub fail: bool,
}

impl MockProcs {
    pub fn with(pids: &[u32]) -> Self {
        Self { running: Mutex::new(pids.iter().map(|p| (*p, "Normal".to_string())).collect()), ..Default::default() }
    }
}

impl ProcessOps for MockProcs {
    fn request_close(&self, pid: u32) -> anyhow::Result<u32> {
        if self.fail {
            anyhow::bail!("mock close failed");
        }
        Ok(if self.running.lock().remove(&pid).is_some() {
            self.closed.lock().push(pid);
            1
        } else {
            0
        })
    }
    fn terminate(&self, pid: u32) -> anyhow::Result<()> {
        if self.fail {
            anyhow::bail!("mock terminate failed");
        }
        self.running.lock().remove(&pid).map(|_| ()).ok_or_else(|| anyhow::anyhow!("not running"))
    }
    fn priority(&self, pid: u32) -> anyhow::Result<String> {
        self.running.lock().get(&pid).cloned().ok_or_else(|| anyhow::anyhow!("not running"))
    }
    fn set_priority(&self, pid: u32, class: &str) -> anyhow::Result<()> {
        crate::frameforge::processes::check_priority(class)?;
        if self.fail {
            anyhow::bail!("mock priority failed");
        }
        match self.running.lock().get_mut(&pid) {
            Some(p) => {
                *p = class.to_string();
                Ok(())
            }
            None => anyhow::bail!("not running"),
        }
    }
    fn is_running(&self, pid: u32) -> bool {
        self.running.lock().contains_key(&pid)
    }
}
