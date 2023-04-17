const {createApp} = Vue;

createApp({
    data() {
        return {
            logs: "",
            status: false,
            socket: null,
            reconnector: null,
            current: 0,
            total: 0,
            failed: 0,
            transfer_speed_history: [],
            size_estimate: 0,
            bytes_written: 0,
            bytes_sent: 0,
            bytes_sent_prev: 0,
            bytes_sent_prev_time: 0,
            peers: [],
            curr_file: "Starting...",
            settings_editable: true,
            backup_running: false,
            backup_msg: "",
            last_backup_success: null,
            starting: false,
            crash_message: "",
            restoring: false,
            configuration: {
                path: ""
            }
        }
    },
    computed: {
        percent() {
            if (this.total === 0) return 0;

            let base_files = (this.current / this.total);
            let base_network = (this.bytes_sent / this.bytes_written);

            return Math.max((base_files * 100) - 2, 0);
        },
        transfer_speed() {
            let sum = 0;
            this.transfer_speed_history.forEach((val) => sum += val)

            let avg = sum / this.transfer_speed_history.length

            if (isNaN(avg) || !isFinite(avg)) {
                return "unknown"
            }

            return `${this.bytes_to_human(avg)}/s`
        },
        data_to_send() {
            return this.bytes_to_human(Math.max(this.bytes_written - this.bytes_sent), 0)
        }
    },
    methods: {
        bytes_to_human(bytes) {
            let units = ["B", "KiB", "MiB", "GiB", "TiB"];
            let unit = 0;

            while (bytes >= 1024 && unit < units.length) {
                bytes /= 1024;
                unit++;
            }

            return `${bytes.toFixed(unit > 2 ? 1 : 2)} ${units[unit]}`;
        },
        transfer_speed_bytes() {
            let timespan = Date.now() - this.bytes_sent_prev_time;
            let data_transferred = this.bytes_sent - this.bytes_sent_prev;
            let bytes_per_second = (data_transferred / timespan) * 1000;

            return bytes_per_second;
        },
        get_config() {
            if (this.socket) {
                this.socket.send(JSON.stringify({
                    type: "GetConfig"
                }));
            }
        },
        start_restore() {
            if (this.socket) {
                if (this.settings_editable) {
                    this.send_config();
                }

                this.socket.send(JSON.stringify({
                    type: "StartRestore"
                }));

                this.settings_editable = false;
            }
        },
        send_config() {
            if (this.socket) {
                this.socket.send(JSON.stringify({
                    type: "Config",
                    data: this.configuration
                }));
            }
        },
        start_backup() {
            if (this.socket) {
                if (this.settings_editable) {
                    this.send_config();
                }

                this.socket.send(JSON.stringify({
                    type: "StartBackup"
                }));
            }
        },
        connect_ws() {
            this.socket = new WebSocket(`ws://${window.location.host}/ws`);

            this.socket.addEventListener('message', (event) => {
                let message = JSON.parse(event.data);

                if (message["type"] === "Progress") {
                    this.current = message["data"].current;
                    this.total = message["data"].total;
                    this.curr_file = message["data"].file;
                    this.failed = message["data"].failed;

                    this.size_estimate = message["data"].size_estimate;
                    this.bytes_written = message["data"].bytes_written;

                    this.bytes_sent_prev = this.bytes_sent;
                    this.bytes_sent = message["data"].bytes_sent;
                    this.bytes_sent_prev_time = Date.now();

                    let speed = this.transfer_speed_bytes();
                    if (!isNaN(speed) && isFinite(speed)) {
                        this.transfer_speed_history.push(speed);
                    }

                    if (this.transfer_speed_history.length > 25) {
                        this.transfer_speed_history.shift();
                    }

                    if (message["data"].peers !== null)
                        this.peers = message["data"].peers;

                    this.starting = false;
                    this.backup_running = true;
                } else if (message["type"] === "Message") {
                    this.logs += message["data"] + "\n";
                    let el = document.querySelector("#logs");
                    el.scrollTo(0, el.scrollHeight);
                } else if (message["type"] === "BackupFinished") {
                    this.last_backup_success = message["data"][0];
                    this.backup_msg = message["data"][1];
                    this.total = 0;
                    this.current = 0;
                    this.bytes_written = 0;
                    this.size_estimate = 0;
                    this.backup_running = false;
                    this.starting = false;
                } else if (message["type"] === "BackupStarted") {
                    this.bytes_sent = 0;
                    this.failed = 0;
                    this.backup_running = true;
                    this.starting = false;
                } else if (message["type"] === "Config") {
                    this.configuration = message["data"];
                    this.crash_message = ""

                    if (this.configuration.path) {
                        this.settings_editable = false;
                    }
                } else if (message["type"] === "Panic") {
                    this.crash_message = message["data"];
                    this.status = false;
                } else if (message["type"] === "RestoreStarted") {
                    this.restoring = true;
                    this.starting = false;
                } else if (message["type"] === "RestoreFinished") {
                    this.restoring = false;
                }

            });

            this.socket.addEventListener('open', () => {
                this.status = true;
                this.get_config();
                clearInterval(this.reconnctor);
            });

            this.socket.addEventListener('close', () => {
                this.closed_ws();
            });

            this.socket.addEventListener('error', () => {
                this.closed_ws();
            });
        },
        closed_ws() {
            this.socket = null;
            this.status = false;
            this.backup_running = false;
            this.current = 0;
            this.total = 0;
            this.failed = 0;
            this.size_estimate = 0;
            this.bytes_written = 0;
            this.bytes_sent = 0;
            this.curr_file = "Starting...";
            this.restoring = false;

            clearInterval(this.reconnctor);
            this.reconnctor = setInterval(() => {
                this.connect_ws()
            }, 1000);
        },
        send_ws(msg) {
            if (this.socket) {
                this.socket.send(msg);
            }
        }
    },
    mounted() {
        document.querySelector("#app").style.display = "block";
        this.connect_ws();
    }
}).mount('#app');
