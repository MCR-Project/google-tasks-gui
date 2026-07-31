# gtasks-tui

---

layout: default
title: Privacy Policy
---

# Privacy Policy for gtasks-tui

**Last Updated:** July 31, 2026

This Privacy Policy applies to the **gtasks-tui** application, developed and maintained by **MCR-Project** ("we", "us", or "our"). This document outlines how gtasks-tui accesses, uses, stores, and protects your information when you use our application.

---

## 1. Information We Access & Collect

gTasks is a **local-first desktop and terminal application** designed to manage your Google Tasks. To provide its core features, gTasks requests access to your Google Account data via the official Google Tasks REST API (`https://www.googleapis.com/auth/tasks`).

Specifically, gTasks accesses:

* **Google Tasks Data:** Task list titles, task titles, task descriptions/notes, completion statuses, due dates, subtask hierarchies, and modification timestamps.
* **OAuth Authentication Tokens:** Access and refresh tokens issued by Google to authenticate your session.

We **do not** collect or access your name, email address, password, contacts, Google Drive files, or any other personal information associated with your Google Account.

---

## 2. How We Use Your Data

All data accessed through the Google Tasks API is used **strictly to provide and improve user-facing features within the application**. Specifically, gTasks uses your data to:

* Display your task lists and tasks in the desktop (GTK4/Libadwaita) and terminal (Ratatui) interfaces.
* Create, edit, complete, reorder, or delete tasks on your behalf based on your direct in-app actions.
* Synchronize local offline changes (stored in SQLite) with your remote Google Tasks account.

### 🚫 Prohibited Uses & Explicit Disclaimers

* **No Selling:** We do not sell, rent, or trade your Google user data to any third party under any circumstances.
* **No Advertising:** Your data is never used for advertising, marketing, or behavioral tracking purposes.
* **No AI/ML Training:** Google Workspace user data accessed by gTasks is **never** used to develop, improve, or train generalized or non-personalized Artificial Intelligence (AI) or Machine Learning (ML) models.
* **No Server Transfers:** gTasks does not transmit your tasks or authentication tokens to any external server managed by MCR-Project or any third party. All communication occurs directly between your local device and Google's official endpoints (`googleapis.com`).

---

## 3. Data Storage & Security

gTasks prioritizes data minimization and local security:

* **Local Database Storage:** Your task lists and tasks are cached locally on your device in a SQLite database (`task_lists.db`) to enable fast offline access.
* **OAuth Token Security:** OAuth 2.0 access and refresh tokens are encrypted and stored using your operating system’s secure credential storage system (e.g., Linux Secret Service / KWallet via `keyring`).
* **Transport Security:** All communication between gTasks and Google API servers is strictly encrypted in transit using standard HTTPS/TLS protocols.

---

## 4. How to Manage or Delete Your Data

Because gTasks operates locally on your computer, you have total control over your data:

1. **Delete Local Data:** You can remove all locally stored task caches and databases at any time by deleting the application's local configuration directory (`~/.config/gtasks/` or `~/.local/share/gtasks/`).
2. **Remove Account Access:** You can revoke gTasks' access to your Google Account at any time by visiting your [Google Account Permissions Page](https://myaccount.google.com/permissions) and selecting **Remove Access** for gTasks.
3. **Delete Tasks:** Deleting a task or task list within the gTasks interface will permanently issue a deletion command to your Google Tasks account.

---

## 5. Third-Party Services

gTasks interacts directly with **Google LLC** services. Your use of gTasks is subject to Google's own policies regarding task data:

* [Google Privacy Policy](https://policies.google.com/privacy)
* [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy)

---

## 6. Open Source & Transparency

gTasks is open-source software distributed under the MIT License. You can review the complete source code to verify our data security practices at our official repository:
👉 [https://github.com/MCR-Project/google-tasks-gui](https://github.com/MCR-Project/google-tasks-gui)

---

## 7. Contact Us

If you have any questions or concerns regarding this Privacy Policy or gTasks' data handling practices, please open an issue on our GitHub repository or contact us:

* **Developer / Organization:** MCR-Project
* **GitHub Repository:** [https://github.com/MCR-Project/google-tasks-gui](https://github.com/MCR-Project/google-tasks-gui)
* **Contact Email:** `alexis.insalaco@gmail.com`
