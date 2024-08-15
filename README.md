# Mini METARs

Mini METARs is a micro-utility to display up-to-date METAR information (primarily altimeter and wind direction + speed,
with full METAR toggle-able) and VATSIM ATIS code for a number of user-inputted airports/stations in a minimal on-top
window.

Built with Tauri, with a Rust backend for METAR fetching and a SolidJS frontend for UI actions.

![image](https://github.com/user-attachments/assets/989b103b-64f5-4d43-89ef-c9c60962ddd0)

## Features

### Minimal always-on-top window

The application window stays "on top" of other windows for constant visibility, and expands or contracts as needed to
display more or less information.

Clicking on an ATIS letter will toggle visibility of the full VATSIM ATIS text, and clicking on either the altimeter
setting or the wind
will toggle visibility of the full METAR text (note: only one of the ATIS and METAR full text will be visible at once).

### Visibility controls

You can toggle visibility of the titlebar (Windows-only) and the input box with the following shortcuts:

* `Ctrl/Cmd` + `D`: toggle visibility of input box and station delete icons
* `Ctrl/Cmd` + `B`: toggle visibility of the titlebar (Windows only)
* `Ctrl/Cmd` + `M`: minimize window

### Profiles

Mini METARs supports loading and saving profiles, which include the list of stations, the size and position of the
window, and the visibility state (visible or hidden) of the input box and titlebar.

By default, Mini METARs will load your last used profile on application startup.

The following shortcuts allow you to work with profiles:

* `Ctrl/Cmd` + `S`: save current profile, either to existing location (if you've loaded a profile) or to a new location
  if the current profile is new
* `Ctrl/Cmd` + `Shift` + `S`: "save as" current profile
* `Ctrl/Cmd` + `O`: open profile

Profiles also store the altimeter setting units. Use `Ctrl/Cmd` + `U` to toggle between inHg and hPa.

## FAQ

**How often do METARs update**?

* Each airport/station checks for a METAR update every 2 to 2.5 minutes, with the value slightly randomized to prevent
  "clumping" of requests.

**How often do VATSIM ATIS codes update?**

* Each airport/station checks for a VATSIM ATIS code update every 20 to 30 seconds.

**What if an airport has separate arrival and departure ATIS?**

* Both codes will be displayed in the format "`ARRIVAL_CODE`/`DEPARTURE_CODE`"
