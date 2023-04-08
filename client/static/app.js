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
            curr_file: "Starting...",
            settings_editable: true,
            backup_running: false,
            backup_msg: "",
            last_backup_success: null,
            starting: false,
            crash_message: "",
            configuration: {
                path: ""
            }
        }
    },
    computed: {
        percent() {
            if (this.total === 0) return 0;
            return Math.max((this.current / this.total) * 100 - 1, 0);
        }
    },
    methods: {
        get_config() {
            if (this.socket) {
                this.socket.send(JSON.stringify({
                    type: "GetConfig"
                }));
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
                    this.backup_running = false;
                    this.starting = false;
                } else if (message["type"] === "BackupStarted") {
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
