PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
APPLICATIONS_DIR ?= $(DATADIR)/applications
ICON_DIR ?= $(DATADIR)/icons/hicolor/scalable/apps
METAINFO_DIR ?= $(DATADIR)/metainfo
SYSTEMD_USER_DIR ?= $(HOME)/.config/systemd/user
AUTOSTART_DIR ?= $(HOME)/.config/autostart

.PHONY: install-user uninstall-user check-user-surface build-bin lab lab-smoke lan-test web-install web-dev web-build web-check

install-user: target/release/ticonnect
	install -Dm755 target/release/ticonnect $(BINDIR)/ticonnect
	install -d $(SYSTEMD_USER_DIR) $(AUTOSTART_DIR) $(APPLICATIONS_DIR) $(ICON_DIR) $(METAINFO_DIR)
	install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.desktop $(APPLICATIONS_DIR)/org.ticonnect.TiConnect.desktop
	install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.svg $(ICON_DIR)/org.ticonnect.TiConnect.svg
	install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml $(METAINFO_DIR)/org.ticonnect.TiConnect.metainfo.xml
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnectd.service > $(SYSTEMD_USER_DIR)/ticonnectd.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnect-overlay.service > $(SYSTEMD_USER_DIR)/ticonnect-overlay.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnect-ui.service > $(SYSTEMD_USER_DIR)/ticonnect-ui.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect.desktop > $(AUTOSTART_DIR)/ticonnect.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-overlay.desktop > $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-ui.desktop > $(AUTOSTART_DIR)/ticonnect-ui.desktop
	chmod 644 $(SYSTEMD_USER_DIR)/ticonnectd.service $(SYSTEMD_USER_DIR)/ticonnect-overlay.service $(SYSTEMD_USER_DIR)/ticonnect-ui.service $(AUTOSTART_DIR)/ticonnect.desktop $(AUTOSTART_DIR)/ticonnect-overlay.desktop $(AUTOSTART_DIR)/ticonnect-ui.desktop

target/release/ticonnect:
	cargo build --release --bin ticonnect

target/debug/ticonnect:
	CARGO_PROFILE_DEV_DEBUG=0 cargo build --bin ticonnect

build-bin:
	@if [ -n "$(TICONNECT_BIN)" ]; then \
		test -x "$(TICONNECT_BIN)" || { echo "TICONNECT_BIN is not executable: $(TICONNECT_BIN)" >&2; exit 1; }; \
		echo "using TICONNECT_BIN=$(TICONNECT_BIN)"; \
	else \
		allow="$(TICONNECT_ALLOW_UNDEFINED)"; \
		if [ -z "$$allow" ] && [ -n "$${NIX_LD:-}" ]; then allow=1; fi; \
		if [ "$$allow" = "1" ]; then \
			CARGO_PROFILE_DEV_DEBUG=0 cargo rustc -p ticonnect --bin ticonnect -- -C link-arg=-Wl,--allow-shlib-undefined; \
		else \
			CARGO_PROFILE_DEV_DEBUG=0 cargo build --bin ticonnect; \
		fi; \
	fi

lab: build-bin
	TICONNECT_BIN="$(TICONNECT_BIN)" tests/local/lab.sh

lab-smoke: build-bin
	TICONNECT_BIN="$(TICONNECT_BIN)" tests/local/lab.sh --smoke

uninstall-user:
	rm -f $(BINDIR)/ticonnect
	rm -f $(APPLICATIONS_DIR)/org.ticonnect.TiConnect.desktop
	rm -f $(ICON_DIR)/org.ticonnect.TiConnect.svg
	rm -f $(METAINFO_DIR)/org.ticonnect.TiConnect.metainfo.xml
	rm -f $(SYSTEMD_USER_DIR)/ticonnectd.service
	rm -f $(SYSTEMD_USER_DIR)/ticonnect-overlay.service
	rm -f $(SYSTEMD_USER_DIR)/ticonnect-ui.service
	rm -f $(AUTOSTART_DIR)/ticonnect.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-ui.desktop

check-user-surface: target/release/ticonnect
	@if target/release/ticonnect --help | grep -E '^[[:space:]]*(daemon|overlay|cli)\b'; then \
		echo "internal command exposed in user help" >&2; \
		exit 1; \
	fi

lan-test:
	tests/lan/run.sh

WEB_DIR ?= crates/ui/web
UI_ASSETS ?= crates/ui/assets

web-install:
	cd $(WEB_DIR) && bun install --frozen-lockfile

web-dev:
	cd $(WEB_DIR) && bun run dev

web-build:
	cd $(WEB_DIR) && bun run build
	rm -rf $(UI_ASSETS)
	mkdir -p $(UI_ASSETS)
	cp -R $(WEB_DIR)/dist/. $(UI_ASSETS)/

web-check:
	cd $(WEB_DIR) && bun run check && bun run test
