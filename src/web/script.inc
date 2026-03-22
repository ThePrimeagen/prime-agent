      const autosaveState = new Map();
      let primeSocket = null;
      const primeWsQueue = [];

      function parsePrimeGenerationsEl(el) {
        if (!el || !el.textContent) {
          return { skills: {}, pipelines: {}, list_epoch: 0 };
        }
        try {
          const j = JSON.parse(el.textContent.trim());
          return {
            skills: j.skills || {},
            pipelines: j.pipelines || {},
            list_epoch: j.list_epoch || 0
          };
        } catch (_e) {
          return { skills: {}, pipelines: {}, list_epoch: 0 };
        }
      }

      function setGenerationsFromServer(g) {
        if (!g) {
          return;
        }
        primeGenerations = {
          skills: { ...(g.skills || {}) },
          pipelines: { ...(g.pipelines || {}) },
          list_epoch: g.list_epoch || 0
        };
      }

      function serverGenerationsNewer(client, server) {
        if (!server) {
          return false;
        }
        if ((server.list_epoch || 0) > (client.list_epoch || 0)) {
          return true;
        }
        for (const k of Object.keys(client.skills || {})) {
          if (!Object.prototype.hasOwnProperty.call(server.skills || {}, k)) {
            return true;
          }
        }
        for (const k of Object.keys(client.pipelines || {})) {
          if (!Object.prototype.hasOwnProperty.call(server.pipelines || {}, k)) {
            return true;
          }
        }
        for (const [k, v] of Object.entries(server.skills || {})) {
          if ((client.skills[k] || 0) < v) {
            return true;
          }
        }
        for (const [k, v] of Object.entries(server.pipelines || {})) {
          if ((client.pipelines[k] || 0) < v) {
            return true;
          }
        }
        return false;
      }

      let primeGenerations = parsePrimeGenerationsEl(
        document.getElementById("prime-generations")
      );

      function nextMsgId() {
        if (typeof crypto !== "undefined" && crypto.randomUUID) {
          return crypto.randomUUID();
        }
        return `m-${Date.now()}-${Math.random().toString(16).slice(2, 10)}`;
      }

      function sendPrime(obj) {
        const s = JSON.stringify(obj);
        if (primeSocket && primeSocket.readyState === WebSocket.OPEN) {
          primeSocket.send(s);
        } else {
          primeWsQueue.push(s);
        }
      }

      function flushPrimeQueue() {
        while (
          primeWsQueue.length > 0 &&
          primeSocket &&
          primeSocket.readyState === WebSocket.OPEN
        ) {
          primeSocket.send(primeWsQueue.shift());
        }
      }

      function primeEscHtml(s) {
        return String(s)
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;")
          .replace(/"/g, "&quot;");
      }

      function primeEscAttr(s) {
        return String(s).replace(/&/g, "&amp;").replace(/"/g, "&quot;");
      }

      function primeHiddenAttr(active, tab) {
        return active === tab ? "" : " hidden";
      }

      function primeAriaCurrent(active, tab) {
        return active === tab ? "page" : "false";
      }

      /** Builds `#left-nav` inner HTML from a `type: "ui"` JSON payload (mirrors `render_left_nav_inner`). */
      function buildPrimeLeftNavHtml(data) {
        const a = data.active_section || "";
        const skills = data.skills || [];
        const pipelines = data.pipelines || [];
        const steps = data.pipeline_steps || [];
        const selSkill = data.selected_skill;
        const selPipe = data.selected_pipeline;
        let o = "";
        o +=
          '        <div style="display:flex;gap:0.5rem;margin-bottom:1rem;">\n          <a\n            id="tab-skills"\n            data-testid="tab-skills"\n            data-icon="sword"\n            title="Skills"\n            aria-label="Skills"\n            href="/skills"\n            aria-current="' +
          primeAriaCurrent(a, "skills") +
          '"\n            style="display:inline-flex;align-items:center;justify-content:center;width:2rem;height:2rem;border:1px solid #bbb;border-radius:6px;background:#fff;cursor:pointer;text-decoration:none;color:inherit;">&#9876;</a>\n          <a\n            id="tab-pipeline"\n            data-testid="tab-pipeline"\n            data-icon="pipe"\n            title="Pipeline"\n            aria-label="Pipeline"\n            href="/pipelines"\n            aria-current="' +
          primeAriaCurrent(a, "pipelines") +
          '"\n            style="display:inline-flex;align-items:center;justify-content:center;width:2rem;height:2rem;border:1px solid #bbb;border-radius:6px;background:#fff;cursor:pointer;text-decoration:none;color:inherit;">&#x2554;&#x2557;</a>\n        </div>\n\n        <section id="skills-nav-panel" data-tab-panel="skills"' +
          primeHiddenAttr(a, "skills") +
          ">\n          <h2 style=\"margin-top:0;\">Skills</h2>\n          <button\n            type=\"button\"\n            data-testid=\"skill-create-open\"\n            style=\"background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;\"\n            onclick=\"document.getElementById('skill-modal').showModal();\">[ + ]</button>\n\n          <dialog id=\"skill-modal\">\n            <form data-ws-form=\"create_skill\">\n              <h3>Create Skill</h3>\n              <label>Name<br><input name=\"name\" required></label><br><br>\n              <label>Prompt<br><textarea name=\"prompt\" rows=\"6\" cols=\"40\" required></textarea></label><br><br>\n              <button type=\"submit\">Save</button>\n              <button type=\"button\" onclick=\"document.getElementById('skill-modal').close();\">Cancel</button>\n            </form>\n          </dialog>\n\n";
        if (skills.length > 0) {
          o +=
            '          <ul id="skills-nav-list" style="padding-left:0;margin-top:1rem;">\n';
          for (const skill of skills) {
            const isSel = selSkill && selSkill.name === skill.name;
            o +=
              '            <li style="list-style:none;margin-top:0.5rem;">\n              <a\n                data-testid="skill-nav-link"\n                data-selected="' +
              (isSel ? "true" : "false") +
              '"\n                href="/skills/' +
              skill.name_encoded +
              '"\n                style="display:block;padding:0.4rem 0.5rem;border:1px solid #ddd;border-radius:6px;text-decoration:none;color:inherit;">' +
              primeEscHtml(skill.name) +
              "</a>\n            </li>\n";
          }
          o += "          </ul>\n";
        } else {
          o += "          <p>No skills yet.</p>\n";
        }
        o +=
          '        </section>\n\n        <section id="pipeline-nav-panel" data-tab-panel="pipeline"' +
          primeHiddenAttr(a, "pipelines") +
          ">\n          <h2 style=\"margin-top:0;\">Pipeline</h2>\n          <button\n            type=\"button\"\n            data-testid=\"pipeline-create-open\"\n            style=\"background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;\"\n            onclick=\"document.getElementById('pipeline-modal').showModal();\">[ + ]</button>\n\n          <dialog id=\"pipeline-modal\">\n            <form data-ws-form=\"create_pipeline\">\n              <h3>Create Pipeline</h3>\n              <label>Name<br><input name=\"name\" data-lowercase-field=\"pipeline-name\" required></label><br><br>\n              <button type=\"submit\">Save</button>\n              <button type=\"button\" onclick=\"document.getElementById('pipeline-modal').close();\">Cancel</button>\n            </form>\n          </dialog>\n\n";
        if (pipelines.length > 0) {
          o +=
            '          <ul id="pipeline-nav-list" style="padding-left:0;margin-top:1rem;">\n';
          for (const pl of pipelines) {
            const isSel = selPipe && selPipe.name === pl.name;
            o +=
              '            <li style="list-style:none;margin-top:0.5rem;">\n              <a\n                data-testid="pipeline-nav-link"\n                data-selected="' +
              (isSel ? "true" : "false") +
              '"\n                href="/pipelines/' +
              pl.name_encoded +
              '"\n                style="display:block;padding:0.4rem 0.5rem;border:1px solid #ddd;border-radius:6px;text-decoration:none;color:inherit;">' +
              primeEscHtml(pl.name) +
              "</a>\n            </li>\n";
          }
          o += "          </ul>\n";
        } else {
          o += "          <p>No pipelines yet.</p>\n";
        }
        if (selPipe) {
          o +=
            '          <h3 style="margin-top:1.25rem;margin-bottom:0.25rem;">Steps</h3>\n';
          if (steps.length > 0) {
            o +=
              '          <ul id="pipeline-step-nav-list" style="padding-left:0;margin-top:0.5rem;">\n';
            for (const step of steps) {
              o +=
                '            <li\n              data-testid="pipeline-step-nav-item"\n              data-step-id="' +
                step.id +
                '"\n              data-ws-pipeline="' +
                primeEscAttr(selPipe.name) +
                '"\n              data-ws-reorder="1"\n              draggable="true"\n              style="list-style:none;margin-top:0.4rem;padding:0.45rem 0.55rem;border:1px solid #ddd;border-radius:6px;background:#fff;display:flex;justify-content:space-between;align-items:center;gap:0.5rem;cursor:grab;">\n              <span>' +
                primeEscHtml(step.title) +
                '</span>\n              <span style="display:flex;flex-direction:column;align-items:flex-end;gap:0.1rem;">\n                <span data-testid="pipeline-step-skill-count" style="font-size:0.8rem;color:#4b5563;">' +
                step.skill_count +
                '</span>\n                <span data-testid="pipeline-step-skill-summary" style="font-size:0.72rem;color:#6b7280;">' +
                primeEscHtml(step.skill_summary) +
                "</span>\n              </span>\n            </li>\n";
            }
            o += "          </ul>\n";
          } else {
            o +=
              '          <p style="margin-top:0.5rem;">No steps yet.</p>\n';
          }
        }
        o += "        </section>\n";
        return o;
      }

      /** Builds `#main-content` inner HTML from a `type: "ui"` JSON payload (mirrors `render_main_inner`). */
      function buildPrimeMainHtml(data) {
        const a = data.active_section || "";
        const skills = data.skills || [];
        const pipelines = data.pipelines || [];
        const steps = data.pipeline_steps || [];
        const selSkill = data.selected_skill;
        const selPipe = data.selected_pipeline;
        let o = "";
        o +=
          '        <section id="skills-main-panel" data-tab-main="skills"' +
          primeHiddenAttr(a, "skills") +
          ">\n          <h1>Skills</h1>\n";
        if (selSkill) {
          const sk = selSkill;
          o +=
            '          <article data-skill-editor data-skill-id="' +
            primeEscAttr(sk.name) +
            '" style="max-width:740px;border:1px solid #ddd;padding:0.8rem;margin-top:0.8rem;position:relative;">\n            <form data-skill-form data-ws-form="update_skill" data-old-name="' +
            primeEscAttr(sk.name) +
            '" data-skill-location="/skills/' +
            sk.name_encoded +
            '">\n              <label>Name<br><input name="name" value="' +
            primeEscAttr(sk.name) +
            '" required></label><br><br>\n              <label>Prompt<br><textarea name="prompt" rows="8" cols="70" required>' +
            primeEscHtml(sk.prompt) +
            '</textarea></label><br><br>\n              <div style="position:absolute;top:0.5rem;right:0.6rem;display:flex;align-items:flex-start;gap:0.35rem;">\n                <div data-delete-controls style="position:relative;">\n                  <button\n                    type="button"\n                    data-testid="delete-skill-trigger"\n                    aria-label="Delete skill"\n                    aria-haspopup="dialog"\n                    aria-expanded="false"\n                    title="Delete skill"\n                    style="border:none;background:transparent;color:#dc2626;cursor:pointer;font-size:0.85rem;line-height:1;padding:0;">&#128465;</button>\n                  <div\n                    data-testid="delete-skill-popover"\n                    data-delete-popover\n                    hidden\n                    style="position:absolute;top:1.1rem;right:0;background:#fff;border:1px solid #d1d5db;border-radius:6px;padding:0.45rem;box-shadow:0 2px 8px rgba(0,0,0,0.12);z-index:10;max-width:260px;">\n                    <p data-testid="delete-skill-warning" style="margin:0 0 0.45rem 0;font-size:0.72rem;line-height:1.35;color:#374151;">Delete this skill? This is permanent and removes the skill folder and SKILL.md.</p>\n                    <button\n                      type="button"\n                      data-delete-confirm\n                      data-testid="delete-skill-confirm"\n                      style="border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.1rem 0.35rem;font-size:0.72rem;cursor:pointer;">Delete permanently</button>\n                  </div>\n                </div>\n              </div>\n            </form>\n            <form data-skill-delete-form data-ws-form="delete_skill" data-skill-name="' +
            primeEscAttr(sk.name) +
            '"></form>\n          </article>\n';
        } else if (skills.length > 0) {
          o += '            <p>Select a skill from the left nav.</p>\n';
        } else {
          o += "            <p>No skills yet.</p>\n";
        }
        o +=
          '        </section>\n\n        <section id="pipeline-main-panel" data-tab-main="pipeline"' +
          primeHiddenAttr(a, "pipelines") +
          ">\n";
        if (selPipe) {
          const pl = selPipe;
          o +=
            '          <h1 id="pipeline-title">' +
            primeEscHtml(pl.name) +
            '</h1>\n          <button\n            type="button"\n            data-testid="pipeline-step-create-open"\n            style="background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;"\n            onclick="document.getElementById(\'pipeline-step-modal\').showModal();">Add Step</button>\n\n          <dialog id="pipeline-step-modal">\n            <form data-ws-form="create_step" data-pipeline="' +
            primeEscAttr(pl.name) +
            '">\n              <h3>Create Step</h3>\n              <label>Title<br><input name="title" data-lowercase-field="pipeline-step-title" required></label><br><br>\n              <label>Prompt<br><textarea name="prompt" rows="6" cols="40" required></textarea></label><br><br>\n              <button type="submit">Save</button>\n              <button type="button" onclick="document.getElementById(\'pipeline-step-modal\').close();">Cancel</button>\n            </form>\n          </dialog>\n\n';
          if (steps.length > 0) {
            for (const step of steps) {
              o +=
                '            <article data-testid="pipeline-step-editor" style="max-width:740px;border:1px solid #ddd;padding:0.8rem;margin-top:0.8rem;position:relative;">\n              <form data-ws-form="update_step" data-pipeline="' +
                primeEscAttr(pl.name) +
                '" data-step-id="' +
                step.id +
                '">\n                <label>Title<br><input name="title" data-lowercase-field="pipeline-step-title" value="' +
                primeEscAttr(step.title) +
                '" required></label><br><br>\n                <label>Description<br><textarea name="prompt" rows="6" cols="70" required>' +
                primeEscHtml(step.prompt) +
                '</textarea></label><br><br>\n                <button type="submit" data-testid="pipeline-step-save">Save</button>\n              </form>\n              <div style="display:flex;gap:0.5rem;align-items:center;flex-wrap:wrap;">\n                <form data-ws-form="add_step_skill" data-pipeline="' +
                primeEscAttr(pl.name) +
                '" data-step-id="' +
                step.id +
                '" style="display:flex;gap:0.4rem;align-items:center;">\n                  <select name="skill_id" required>\n                    <option value="">Select skill</option>\n';
              for (const sk of skills) {
                o +=
                  '                    <option value="' +
                  primeEscAttr(sk.name) +
                  '">' +
                  primeEscHtml(sk.name) +
                  "</option>\n";
              }
              o +=
                '                  </select>\n                  <button type="submit">Add Skill</button>\n                </form>\n                <form data-ws-form="delete_step" data-pipeline="' +
                primeEscAttr(pl.name) +
                '" data-step-id="' +
                step.id +
                '">\n                  <button type="submit" data-testid="pipeline-step-delete" style="border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.2rem 0.45rem;font-size:0.8rem;cursor:pointer;">Delete</button>\n                </form>\n              </div>\n';
              if (step.skills && step.skills.length > 0) {
                o +=
                  '              <ul style="padding-left:1.1rem;margin:0.6rem 0 0 0;">\n';
                for (const att of step.skills) {
                  o +=
                    '                <li style="margin-top:0.3rem;">\n                  <span data-testid="pipeline-step-attached-skill">' +
                    primeEscHtml(att.name) +
                    '</span>\n                  <form data-ws-form="delete_step_skill" data-pipeline="' +
                    primeEscAttr(pl.name) +
                    '" data-step-id="' +
                    step.id +
                    '" data-skill-name="' +
                    primeEscAttr(att.name) +
                    '" style="display:inline;">\n                    <button type="submit" style="margin-left:0.4rem;border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.1rem 0.35rem;font-size:0.72rem;cursor:pointer;">Remove</button>\n                  </form>\n                </li>\n';
                }
                o += "              </ul>\n";
              } else {
                o +=
                  '              <p style="margin-top:0.6rem;color:#6b7280;">No skills attached.</p>\n';
              }
              o += "            </article>\n";
            }
          } else {
            o += '            <p>No steps yet.</p>\n';
          }
        } else {
          o += "          <h1>Pipeline</h1>\n";
          if (pipelines.length > 0) {
            o += "          <p>Select a pipeline from the left nav.</p>\n";
          } else {
            o += "          <p>No pipelines yet.</p>\n";
          }
        }
        o += "        </section>\n";
        return o;
      }

      function applyUiBroadcast(data) {
        const nav = document.getElementById("left-nav");
        const main = document.getElementById("main-content");
        const hasView =
          data &&
          Array.isArray(data.skills) &&
          Array.isArray(data.pipelines) &&
          Array.isArray(data.pipeline_steps);
        if (nav && hasView) {
          nav.innerHTML = buildPrimeLeftNavHtml(data);
        }
        if (main && hasView) {
          main.innerHTML = buildPrimeMainHtml(data);
        }
        if (data.push_url) {
          history.replaceState(null, "", data.push_url);
        }
        wireAll();
      }

      async function applyFsRefresh() {
        try {
          const r = await fetch(location.href, {
            cache: "no-store",
            headers: { Accept: "text/html" },
          });
          const text = await r.text();
          const doc = new DOMParser().parseFromString(text, "text/html");
          const genEl = doc.getElementById("prime-generations");
          if (genEl) {
            setGenerationsFromServer(parsePrimeGenerationsEl(genEl));
          }
          const nav = doc.getElementById("left-nav");
          const main = doc.getElementById("main-content");
          const tNav = document.getElementById("left-nav");
          const tMain = document.getElementById("main-content");
          if (nav && tNav) {
            tNav.innerHTML = nav.innerHTML;
          }
          if (main && tMain) {
            tMain.innerHTML = main.innerHTML;
          }
          wireAll();
        } catch (_e) {
          /* ignore */
        }
      }

      function onPrimeMessage(ev) {
        let data;
        try {
          data = JSON.parse(ev.data);
        } catch (_e) {
          return;
        }
        if (data.type === "ui") {
          if (data.generations) {
            setGenerationsFromServer(data.generations);
          }
          applyUiBroadcast(data);
          return;
        }
        if (data.type === "fs_changed") {
          if (!data.generations) {
            void applyFsRefresh();
            return;
          }
          const needFetch = serverGenerationsNewer(primeGenerations, data.generations);
          setGenerationsFromServer(data.generations);
          if (needFetch) {
            void applyFsRefresh();
          }
          return;
        }
        if (Object.prototype.hasOwnProperty.call(data, "ok") && data.id) {
          finishSkillAutosavePending(data.id, data.ok === true);
          /* UI sync comes from broadcast */
        }
      }

      function connectPrimeWs() {
        const protocol = location.protocol === "https:" ? "wss:" : "ws:";
        const url = `${protocol}//${location.host}/ws`;
        primeSocket = new WebSocket(url);
        primeSocket.onopen = () => {
          flushPrimeQueue();
        };
        primeSocket.onmessage = onPrimeMessage;
        primeSocket.onclose = () => {
          setTimeout(connectPrimeWs, 2000);
        };
      }

      function buildSkillPayload(form) {
        const nameInput = form.querySelector("input[name='name']");
        const promptInput = form.querySelector("textarea[name='prompt']");
        if (!nameInput || !promptInput) {
          return null;
        }
        const payload = new URLSearchParams();
        payload.set("name", nameInput.value);
        payload.set("prompt", promptInput.value);
        return payload.toString();
      }

      function getEditorState(editor) {
        const id = editor.getAttribute("data-skill-id") || "";
        if (!autosaveState.has(id)) {
          autosaveState.set(id, {
            inFlight: false,
            lastSentPayload: ""
          });
        }
        return autosaveState.get(id);
      }

      function primeSkillPendingMap() {
        if (!window.__primeSkillAckPending) {
          window.__primeSkillAckPending = new Map();
        }
        return window.__primeSkillAckPending;
      }

      function finishSkillAutosavePending(msgId, ok) {
        const pendingMap = primeSkillPendingMap();
        const pending = pendingMap.get(msgId);
        if (!pending) {
          return;
        }
        pendingMap.delete(msgId);
        if (pending.timeoutId) {
          window.clearTimeout(pending.timeoutId);
        }
        const st = getEditorState(pending.editor);
        st.inFlight = false;
        if (ok) {
          st.lastSentPayload = pending.payload;
        }
      }

      function saveEditor(editor) {
        const state = getEditorState(editor);
        if (state.inFlight) {
          return;
        }

        const form = editor.querySelector("[data-skill-form]");
        if (!form) {
          return;
        }

        const payload = buildSkillPayload(form);
        if (payload === null) {
          return;
        }
        if (payload === state.lastSentPayload) {
          return;
        }

        state.inFlight = true;

        try {
          const nameInput = form.querySelector("input[name='name']");
          const promptInput = form.querySelector("textarea[name='prompt']");
          const oldName = form.getAttribute("data-old-name") || "";
          if (!nameInput || !promptInput) {
            state.inFlight = false;
            return;
          }
          const msgId = nextMsgId();
          const afterPayload = buildSkillPayload(form);
          if (afterPayload === null) {
            state.inFlight = false;
            return;
          }
          const pendingMap = primeSkillPendingMap();
          const timeoutId = window.setTimeout(() => {
            if (pendingMap.has(msgId)) {
              finishSkillAutosavePending(msgId, false);
            }
          }, 2500);
          pendingMap.set(msgId, {
            editor,
            payload: afterPayload,
            timeoutId
          });
          sendPrime({
            op: "update_skill",
            id: msgId,
            old_name: oldName,
            name: nameInput.value,
            prompt: promptInput.value
          });
        } catch (_error) {
          state.inFlight = false;
        }
      }

      function wireAutosave() {
        document.querySelectorAll("[data-skill-editor]").forEach((editor) => {
          const form = editor.querySelector("[data-skill-form]");
          if (!form) {
            return;
          }
          const initial = buildSkillPayload(form);
          if (initial !== null) {
            getEditorState(editor).lastSentPayload = initial;
          }
        });
        if (!window.__primeAutosaveInterval) {
          window.__primeAutosaveInterval = window.setInterval(() => {
            document.querySelectorAll("[data-skill-editor]").forEach((editor) => {
              const form = editor.querySelector("[data-skill-form]");
              if (!form) {
                return;
              }
              const p = buildSkillPayload(form);
              if (p === null) {
                return;
              }
              const st = getEditorState(editor);
              if (st.inFlight) {
                return;
              }
              if (p === st.lastSentPayload) {
                return;
              }
              saveEditor(editor);
            });
          }, 1000);
        }
      }

      function closeDeletePopovers(exceptControl) {
        const controls = document.querySelectorAll("[data-delete-controls]");
        for (const control of controls) {
          if (exceptControl && control === exceptControl) {
            continue;
          }
          const trigger = control.querySelector("[data-testid='delete-skill-trigger']");
          const popover = control.querySelector("[data-delete-popover]");
          if (trigger) {
            trigger.setAttribute("aria-expanded", "false");
          }
          if (popover) {
            popover.hidden = true;
          }
        }
      }

      function wireDeleteConfirmations() {
        const editors = document.querySelectorAll("[data-skill-editor]");
        for (const editor of editors) {
          if (editor.dataset.primeDeleteWired === "1") {
            continue;
          }
          editor.dataset.primeDeleteWired = "1";
          const control = editor.querySelector("[data-delete-controls]");
          const trigger = editor.querySelector("[data-testid='delete-skill-trigger']");
          const popover = editor.querySelector("[data-delete-popover]");
          const confirm = editor.querySelector("[data-delete-confirm]");
          const form = editor.querySelector("[data-skill-delete-form]");
          if (!control || !trigger || !popover || !confirm || !form) {
            continue;
          }

          trigger.addEventListener("click", (event) => {
            event.stopPropagation();
            const open = popover.hidden;
            closeDeletePopovers(control);
            popover.hidden = !open;
            trigger.setAttribute("aria-expanded", open ? "true" : "false");
          });

          popover.addEventListener("click", (event) => {
            event.stopPropagation();
          });

          confirm.addEventListener("click", (event) => {
            event.stopPropagation();
            closeDeletePopovers();
            const skillName = form.getAttribute("data-skill-name") || "";
            sendPrime({
              op: "delete_skill",
              id: nextMsgId(),
              name: skillName
            });
          });
        }

        if (!window.__primeDeleteOutside) {
          window.__primeDeleteOutside = true;
          document.addEventListener("click", (event) => {
            for (const control of document.querySelectorAll("[data-delete-controls]")) {
              if (control.contains(event.target)) {
                return;
              }
            }
            closeDeletePopovers();
          });
        }
      }

      function wirePipelineStepReorder() {
        const list = document.getElementById("pipeline-step-nav-list");
        if (!list) {
          return;
        }
        if (list.dataset.primeReorderWired === "1") {
          return;
        }
        list.dataset.primeReorderWired = "1";
        let dragging = null;
        const rows = list.querySelectorAll("[data-testid='pipeline-step-nav-item']");
        for (const row of rows) {
          row.addEventListener("dragstart", () => {
            dragging = row;
            row.style.opacity = "0.6";
          });
          row.addEventListener("dragend", () => {
            row.style.opacity = "1";
            dragging = null;
          });
          row.addEventListener("dragover", (event) => {
            event.preventDefault();
          });
          row.addEventListener("drop", (event) => {
            event.preventDefault();
            if (!dragging || dragging === row) {
              return;
            }
            const draggingID = dragging.getAttribute("data-step-id");
            const targetID = row.getAttribute("data-step-id");
            const pipeline = dragging.getAttribute("data-ws-pipeline");
            if (!draggingID || !targetID || !pipeline) {
              return;
            }

            list.insertBefore(dragging, row);
            const stepId = parseInt(draggingID, 10);
            const targetStepId = parseInt(targetID, 10);
            sendPrime({
              op: "reorder_step",
              id: nextMsgId(),
              pipeline: pipeline,
              step_id: stepId,
              target_step_id: targetStepId
            });
          });
        }
      }

      function wireWsFormsOnce() {
        if (window.__primeWireWsForms) {
          return;
        }
        window.__primeWireWsForms = true;
        document.addEventListener("submit", (e) => {
          const form = e.target;
          if (!(form instanceof HTMLFormElement)) {
            return;
          }
          const ws = form.getAttribute("data-ws-form");
          if (!ws) {
            return;
          }
          e.preventDefault();
          const id = nextMsgId();
          if (ws === "create_skill") {
            const fd = new FormData(form);
            sendPrime({
              op: "create_skill",
              id: id,
              name: String(fd.get("name") || ""),
              prompt: String(fd.get("prompt") || "")
            });
            form.closest("dialog")?.close();
            return;
          }
          if (ws === "create_pipeline") {
            const fd = new FormData(form);
            sendPrime({
              op: "create_pipeline",
              id: id,
              name: String(fd.get("name") || "")
            });
            form.closest("dialog")?.close();
            return;
          }
          if (ws === "create_step") {
            const fd = new FormData(form);
            const pipeline = form.getAttribute("data-pipeline") || "";
            sendPrime({
              op: "create_step",
              id: id,
              pipeline: pipeline,
              title: String(fd.get("title") || ""),
              prompt: String(fd.get("prompt") || "")
            });
            form.closest("dialog")?.close();
            return;
          }
          if (ws === "update_step") {
            const fd = new FormData(form);
            const pipeline = form.getAttribute("data-pipeline") || "";
            const stepId = parseInt(form.getAttribute("data-step-id") || "0", 10);
            sendPrime({
              op: "update_step",
              id: id,
              pipeline: pipeline,
              step_id: stepId,
              title: String(fd.get("title") || ""),
              prompt: String(fd.get("prompt") || "")
            });
            return;
          }
          if (ws === "delete_step") {
            const pipeline = form.getAttribute("data-pipeline") || "";
            const stepId = parseInt(form.getAttribute("data-step-id") || "0", 10);
            sendPrime({
              op: "delete_step",
              id: id,
              pipeline: pipeline,
              step_id: stepId
            });
            return;
          }
          if (ws === "add_step_skill") {
            const fd = new FormData(form);
            const pipeline = form.getAttribute("data-pipeline") || "";
            const stepId = parseInt(form.getAttribute("data-step-id") || "0", 10);
            sendPrime({
              op: "add_step_skill",
              id: id,
              pipeline: pipeline,
              step_id: stepId,
              skill_id: String(fd.get("skill_id") || "")
            });
            return;
          }
          if (ws === "delete_step_skill") {
            const pipeline = form.getAttribute("data-pipeline") || "";
            const stepId = parseInt(form.getAttribute("data-step-id") || "0", 10);
            const skillName = form.getAttribute("data-skill-name") || "";
            sendPrime({
              op: "delete_step_skill",
              id: id,
              pipeline: pipeline,
              step_id: stepId,
              skill_name: skillName
            });
            return;
          }
        });
      }

      function normalizeLowercaseField(field) {
        const nextValue = field.value.toLowerCase();
        if (field.value !== nextValue) {
          field.value = nextValue;
        }
      }

      function wireLowercaseInputs() {
        const lowercaseFields = document.querySelectorAll(
          "input[data-lowercase-field='pipeline-name'], input[data-lowercase-field='pipeline-step-title']"
        );
        for (const field of lowercaseFields) {
          if (field.dataset.primeLowerWired === "1") {
            continue;
          }
          field.dataset.primeLowerWired = "1";
          normalizeLowercaseField(field);
          field.addEventListener("input", () => {
            normalizeLowercaseField(field);
          });
        }
      }

      function wireAll() {
        wireLowercaseInputs();
        wireAutosave();
        wireDeleteConfirmations();
        wirePipelineStepReorder();
        wireWsFormsOnce();
      }

      connectPrimeWs();
      wireAll();
