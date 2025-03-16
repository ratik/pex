# 📊 cryPto EXporter

This is a Rust-based monitoring tool that collects and exports financial and blockchain-related metrics to Prometheus and Grafana. It features a modular adapter system that allows fetching data from multiple sources, such as Cosmos Bank, Compound, and ERC-20 contracts.

---

## 🚀 Features
- **Modular Adapter System** – Supports different data sources (Cosmos Bank, Compound, ERC-20, etc.).
- **Prometheus Integration** – Exposes `/metrics` endpoint for Prometheus scraping.
- **Concurrency Control** – Has parallel execution.

---

### Configuration
Find example config in config.sample.toml file.
---

### Running in Docker

```sh
docker run -v /root/config.toml:/config.toml -p 9100:9100 -d ratik/pex
```


---

## 📜 License
This project is licensed under the **GPL3 License**.

---

## 🤝 Contributing
I do highly appreciate any contributions. If you have any ideas or suggestions, please feel free to open an issue or a pull request.

---


