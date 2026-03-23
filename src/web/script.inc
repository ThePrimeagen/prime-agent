      const autosaveState = new Map();
      const pipelineRenameFormState = new Map();
      const pipelineStepFormState = new Map();
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

      const PRIME_LAST_SKILL_KEY = "prime.lastSkillName";
      const PRIME_LAST_PIPELINE_KEY = "prime.lastPipelineName";

      function persistLastViewedFromLocation() {
        try {
          const path = location.pathname;
          const sm = path.match(/^\/skills\/([^/]+)\/?$/);
          if (sm) {
            localStorage.setItem(PRIME_LAST_SKILL_KEY, decodeURIComponent(sm[1]));
          }
          const pm = path.match(/^\/pipelines\/([^/]+)/);
          if (pm) {
            localStorage.setItem(
              PRIME_LAST_PIPELINE_KEY,
              decodeURIComponent(pm[1])
            );
          }
        } catch (_e) {
          /* ignore */
        }
      }

      function syncLastViewedFromUi(data) {
        if (!data) {
          return;
        }
        try {
          if (data.selected_skill && data.selected_skill.name) {
            localStorage.setItem(PRIME_LAST_SKILL_KEY, data.selected_skill.name);
          }
          if (data.selected_pipeline && data.selected_pipeline.name) {
            localStorage.setItem(
              PRIME_LAST_PIPELINE_KEY,
              data.selected_pipeline.name
            );
          }
        } catch (_e) {
          /* ignore */
        }
      }

      function findNavLinkHrefForSkill(name) {
        const encoded = encodeURIComponent(name);
        const expected = "/skills/" + encoded;
        const list = document.getElementById("skills-nav-list");
        if (!list) {
          return null;
        }
        for (const a of list.querySelectorAll("a[href]")) {
          if (a.getAttribute("href") === expected) {
            return expected;
          }
        }
        return null;
      }

      function findNavLinkHrefForPipeline(name) {
        const encoded = encodeURIComponent(name);
        const expected = "/pipelines/" + encoded;
        const list = document.getElementById("pipeline-nav-list");
        if (!list) {
          return null;
        }
        for (const a of list.querySelectorAll("a[href]")) {
          if (a.getAttribute("href") === expected) {
            return expected;
          }
        }
        return null;
      }

      function maybeRedirectBareSectionUrl() {
        try {
          const path = location.pathname;
          if (path === "/skills") {
            const raw = localStorage.getItem(PRIME_LAST_SKILL_KEY);
            if (!raw) {
              return;
            }
            const href = findNavLinkHrefForSkill(raw);
            if (!href) {
              localStorage.removeItem(PRIME_LAST_SKILL_KEY);
              return;
            }
            location.replace(href + location.search + location.hash);
            return;
          }
          if (path === "/pipelines") {
            const raw = localStorage.getItem(PRIME_LAST_PIPELINE_KEY);
            if (!raw) {
              return;
            }
            const href = findNavLinkHrefForPipeline(raw);
            if (!href) {
              localStorage.removeItem(PRIME_LAST_PIPELINE_KEY);
              return;
            }
            location.replace(href + location.search + location.hash);
          }
        } catch (_e) {
          /* ignore */
        }
      }

      function findSkillEditorRoot(skillId) {
        if (!skillId) {
          return document.querySelector("[data-skill-editor]");
        }
        for (const ed of document.querySelectorAll("[data-skill-editor]")) {
          if (ed.getAttribute("data-skill-id") === skillId) {
            return ed;
          }
        }
        return document.querySelector("[data-skill-editor]");
      }

      function findUpdateStepForm(pipeline, stepId) {
        const want = String(stepId);
        for (const f of document.querySelectorAll(
          "form[data-ws-form='update_step']"
        )) {
          if (
            f.getAttribute("data-pipeline") === pipeline &&
            f.getAttribute("data-step-id") === want
          ) {
            return f;
          }
        }
        return null;
      }

      function captureShellScrollState() {
        const nav = document.getElementById("left-nav");
        const main = document.getElementById("main-content");
        return {
          windowScrollX: window.scrollX,
          windowScrollY: window.scrollY,
          navScrollTop: nav ? nav.scrollTop : 0,
          navScrollLeft: nav ? nav.scrollLeft : 0,
          mainScrollTop: main ? main.scrollTop : 0,
          mainScrollLeft: main ? main.scrollLeft : 0
        };
      }

      function restoreShellScrollState(shell) {
        if (!shell) {
          return;
        }
        window.scrollTo(shell.windowScrollX, shell.windowScrollY);
        const nav = document.getElementById("left-nav");
        const main = document.getElementById("main-content");
        if (nav) {
          nav.scrollTop = shell.navScrollTop;
          nav.scrollLeft = shell.navScrollLeft;
        }
        if (main) {
          main.scrollTop = shell.mainScrollTop;
          main.scrollLeft = shell.mainScrollLeft;
        }
      }

      function primeFocusNoScroll(el) {
        if (!el || typeof el.focus !== "function") {
          return;
        }
        try {
          el.focus({ preventScroll: true });
        } catch (_e) {
          el.focus();
        }
      }

      function captureSkillEditorFocusState() {
        const ae = document.activeElement;
        if (!ae || !ae.matches || !ae.closest("[data-skill-editor]")) {
          return null;
        }
        const editor = ae.closest("[data-skill-editor]");
        const skillId = editor ? editor.getAttribute("data-skill-id") || "" : "";
        let start = 0;
        let end = 0;
        if (
          ae.matches("input[name='name']") &&
          ae instanceof HTMLInputElement
        ) {
          if (typeof ae.selectionStart === "number") {
            start = ae.selectionStart;
          }
          if (typeof ae.selectionEnd === "number") {
            end = ae.selectionEnd;
          }
          return {
            kind: "skillName",
            skillId: skillId,
            selectionStart: start,
            selectionEnd: end
          };
        }
        if (
          ae.matches("textarea[name='prompt']") &&
          ae instanceof HTMLTextAreaElement
        ) {
          if (typeof ae.selectionStart === "number") {
            start = ae.selectionStart;
          }
          if (typeof ae.selectionEnd === "number") {
            end = ae.selectionEnd;
          }
          return {
            kind: "skillPrompt",
            skillId: skillId,
            selectionStart: start,
            selectionEnd: end,
            scrollTop: ae.scrollTop,
            scrollLeft: ae.scrollLeft
          };
        }
        return null;
      }

      function restoreSkillEditorFocusState(state) {
        if (!state || (state.kind !== "skillName" && state.kind !== "skillPrompt")) {
          return;
        }
        const root = findSkillEditorRoot(state.skillId || "");
        if (!root) {
          return;
        }
        const sel =
          state.kind === "skillName"
            ? "input[name='name']"
            : "textarea[name='prompt']";
        const el = root.querySelector(sel);
        if (
          !el ||
          (!(el instanceof HTMLInputElement) &&
            !(el instanceof HTMLTextAreaElement))
        ) {
          return;
        }
        primeFocusNoScroll(el);
        try {
          if (
            typeof state.selectionStart === "number" &&
            typeof state.selectionEnd === "number"
          ) {
            el.setSelectionRange(state.selectionStart, state.selectionEnd);
          }
        } catch (_e) {
          /* ignore */
        }
        if (
          state.kind === "skillPrompt" &&
          el instanceof HTMLTextAreaElement
        ) {
          if (typeof state.scrollTop === "number") {
            el.scrollTop = state.scrollTop;
          }
          if (typeof state.scrollLeft === "number") {
            el.scrollLeft = state.scrollLeft;
          }
        }
      }

      function capturePipelineRenameFocusState() {
        const ae = document.activeElement;
        if (
          !ae ||
          !ae.matches ||
          !(ae.matches("#pipeline-title") || ae.matches("[data-testid='pipeline-rename-name']"))
        ) {
          return null;
        }
        if (!(ae instanceof HTMLInputElement)) {
          return null;
        }
        let start = 0;
        let end = 0;
        if (typeof ae.selectionStart === "number") {
          start = ae.selectionStart;
        }
        if (typeof ae.selectionEnd === "number") {
          end = ae.selectionEnd;
        }
        return {
          kind: "pipelineRename",
          selectionStart: start,
          selectionEnd: end,
          draftValue: ae.value
        };
      }

      function restorePipelineRenameFocusState(state) {
        if (!state || state.kind !== "pipelineRename") {
          return;
        }
        const el =
          document.getElementById("pipeline-title") ||
          document.querySelector("[data-testid='pipeline-rename-name']");
        if (!el || !(el instanceof HTMLInputElement)) {
          return;
        }
        if (typeof state.draftValue === "string") {
          el.value = state.draftValue;
          normalizeLowercaseField(el);
        }
        primeFocusNoScroll(el);
        try {
          if (
            typeof state.selectionStart === "number" &&
            typeof state.selectionEnd === "number"
          ) {
            el.setSelectionRange(state.selectionStart, state.selectionEnd);
          }
        } catch (_e) {
          /* ignore */
        }
      }

      function resyncPipelineRenameAutosaveBaseline() {
        document.querySelectorAll("form[data-ws-form='rename_pipeline']").forEach((form) => {
          const p = buildPipelineRenamePayload(form);
          if (p !== null) {
            getPipelineRenameFormState(form).lastSentPayload = p;
          }
        });
      }

      function capturePipelineStepEditorFocusState() {
        const ae = document.activeElement;
        if (!ae || !ae.matches) {
          return null;
        }
        const form = ae.closest("form[data-ws-form='update_step']");
        if (!form || !form.closest("[data-testid='pipeline-step-editor']")) {
          return null;
        }
        const pipeline = form.getAttribute("data-pipeline") || "";
        const stepId = form.getAttribute("data-step-id") || "";
        let start = 0;
        let end = 0;
        if (ae.matches("input[name='title']") && ae instanceof HTMLInputElement) {
          if (typeof ae.selectionStart === "number") {
            start = ae.selectionStart;
          }
          if (typeof ae.selectionEnd === "number") {
            end = ae.selectionEnd;
          }
          return {
            kind: "pipelineTitle",
            pipeline: pipeline,
            stepId: stepId,
            selectionStart: start,
            selectionEnd: end,
            draftValue: ae.value
          };
        }
        if (
          ae.matches("textarea[name='prompt']") &&
          ae instanceof HTMLTextAreaElement
        ) {
          if (typeof ae.selectionStart === "number") {
            start = ae.selectionStart;
          }
          if (typeof ae.selectionEnd === "number") {
            end = ae.selectionEnd;
          }
          return {
            kind: "pipelinePrompt",
            pipeline: pipeline,
            stepId: stepId,
            selectionStart: start,
            selectionEnd: end,
            scrollTop: ae.scrollTop,
            scrollLeft: ae.scrollLeft
          };
        }
        if (ae.matches("button[type='submit'], input[type='submit']")) {
          const ta = form.querySelector("textarea[name='prompt']");
          if (ta && ta instanceof HTMLTextAreaElement) {
            if (typeof ta.selectionStart === "number") {
              start = ta.selectionStart;
            }
            if (typeof ta.selectionEnd === "number") {
              end = ta.selectionEnd;
            }
            return {
              kind: "pipelinePrompt",
              pipeline: pipeline,
              stepId: stepId,
              selectionStart: start,
              selectionEnd: end,
              scrollTop: ta.scrollTop,
              scrollLeft: ta.scrollLeft
            };
          }
        }
        return null;
      }

      function restorePipelineStepEditorFocusState(state) {
        if (
          !state ||
          (state.kind !== "pipelineTitle" && state.kind !== "pipelinePrompt")
        ) {
          return;
        }
        const form = findUpdateStepForm(state.pipeline, state.stepId);
        if (!form) {
          return;
        }
        const sel =
          state.kind === "pipelineTitle"
            ? "input[name='title']"
            : "textarea[name='prompt']";
        const el = form.querySelector(sel);
        if (
          !el ||
          (!(el instanceof HTMLInputElement) &&
            !(el instanceof HTMLTextAreaElement))
        ) {
          return;
        }
        if (
          state.kind === "pipelineTitle" &&
          el instanceof HTMLInputElement &&
          typeof state.draftValue === "string"
        ) {
          el.value = state.draftValue;
          normalizeLowercaseField(el);
        }
        primeFocusNoScroll(el);
        try {
          if (
            typeof state.selectionStart === "number" &&
            typeof state.selectionEnd === "number"
          ) {
            el.setSelectionRange(state.selectionStart, state.selectionEnd);
          }
        } catch (_e) {
          /* ignore */
        }
        if (
          state.kind === "pipelinePrompt" &&
          el instanceof HTMLTextAreaElement
        ) {
          const st =
            typeof state.scrollTop === "number" ? state.scrollTop : null;
          const sl =
            typeof state.scrollLeft === "number" ? state.scrollLeft : null;
          const applyScroll = () => {
            if (st !== null) {
              el.scrollTop = st;
            }
            if (sl !== null) {
              el.scrollLeft = sl;
            }
          };
          applyScroll();
          window.requestAnimationFrame(() => {
            window.requestAnimationFrame(applyScroll);
          });
        }
      }

      function wireTabLastViewed() {
        if (window.__primeTabLastViewedWired) {
          return;
        }
        window.__primeTabLastViewedWired = true;
        document.addEventListener(
          "click",
          (ev) => {
            const t = ev.target;
            if (!(t instanceof Element)) {
              return;
            }
            if (
              ev.button !== 0 ||
              ev.ctrlKey ||
              ev.metaKey ||
              ev.shiftKey ||
              ev.altKey
            ) {
              return;
            }
            const skillsTab = t.closest("#tab-skills");
            if (skillsTab && skillsTab.getAttribute("href") === "/skills") {
              const raw = localStorage.getItem(PRIME_LAST_SKILL_KEY);
              if (!raw) {
                return;
              }
              const href = findNavLinkHrefForSkill(raw);
              if (!href) {
                try {
                  localStorage.removeItem(PRIME_LAST_SKILL_KEY);
                } catch (_e) {
                  /* ignore */
                }
                return;
              }
              if (new URL(href, location.origin).pathname === location.pathname) {
                return;
              }
              ev.preventDefault();
              location.assign(href + location.search + location.hash);
              return;
            }
            const pipeTab = t.closest("#tab-pipeline");
            if (pipeTab && pipeTab.getAttribute("href") === "/pipelines") {
              const raw = localStorage.getItem(PRIME_LAST_PIPELINE_KEY);
              if (!raw) {
                return;
              }
              const href = findNavLinkHrefForPipeline(raw);
              if (!href) {
                try {
                  localStorage.removeItem(PRIME_LAST_PIPELINE_KEY);
                } catch (_e) {
                  /* ignore */
                }
                return;
              }
              if (new URL(href, location.origin).pathname === location.pathname) {
                return;
              }
              ev.preventDefault();
              location.assign(href + location.search + location.hash);
            }
          },
          true
        );
      }

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
          ">\n          <h2 style=\"margin-top:0;\">Pipeline</h2>\n          <button\n            type=\"button\"\n            data-testid=\"pipeline-create-open\"\n            style=\"background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;\"\n            onclick=\"document.getElementById('pipeline-modal').showModal();\">[ + ]</button>\n\n          <dialog id=\"pipeline-modal\">\n            <form data-ws-form=\"create_pipeline\">\n              <h3>Create Pipeline</h3>\n              <p data-testid=\"pipeline-create-error\" role=\"alert\" style=\"color:#b91c1c;margin:0.5rem 0;\" hidden></p>\n              <label>Name<br><input name=\"name\" data-lowercase-field=\"pipeline-name\" required></label><br><br>\n              <button type=\"submit\">Save</button>\n              <button type=\"button\" onclick=\"document.getElementById('pipeline-modal').close();\">Cancel</button>\n            </form>\n          </dialog>\n\n";
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
            '          <article data-testid="pipeline-editor" style="max-width:740px;border:1px solid #ddd;padding:0.8rem;margin-top:0.8rem;position:relative;">\n            <form data-ws-form="rename_pipeline" data-old-name="' +
            primeEscAttr(pl.name) +
            '" data-pipeline-location="/pipelines/' +
            pl.name_encoded +
            '">\n              <label>Name<br><input name="name" id="pipeline-title" value="' +
            primeEscAttr(pl.name) +
            '" data-lowercase-field="pipeline-name" data-testid="pipeline-rename-name"></label><br><br>\n              <button type="submit" data-testid="pipeline-rename-save">Rename</button>\n            </form>\n          </article>\n          <button\n            type="button"\n            data-testid="pipeline-step-create-open"\n            style="background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;"\n            onclick="document.getElementById(\'pipeline-step-modal\').showModal();">Add Step</button>\n\n          <dialog id="pipeline-step-modal">\n            <form data-ws-form="create_step" data-pipeline="' +
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
        const shellScroll = captureShellScrollState();
        const skillFocus = captureSkillEditorFocusState();
        const pipelineRenameFocus = capturePipelineRenameFocusState();
        const pipelineFocus = capturePipelineStepEditorFocusState();
        syncLastViewedFromUi(data);
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
        restoreShellScrollState(shellScroll);
        restoreSkillEditorFocusState(skillFocus);
        restorePipelineRenameFocusState(pipelineRenameFocus);
        resyncPipelineRenameAutosaveBaseline();
        restorePipelineStepEditorFocusState(pipelineFocus);
        resyncPipelineStepAutosaveBaseline();
      }

      async function applyFsRefresh() {
        try {
          const shellScroll = captureShellScrollState();
          const skillFocus = captureSkillEditorFocusState();
          const pipelineRenameFocus = capturePipelineRenameFocusState();
          const pipelineFocus = capturePipelineStepEditorFocusState();
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
          restoreShellScrollState(shellScroll);
          restoreSkillEditorFocusState(skillFocus);
          restorePipelineRenameFocusState(pipelineRenameFocus);
          resyncPipelineRenameAutosaveBaseline();
          restorePipelineStepEditorFocusState(pipelineFocus);
          resyncPipelineStepAutosaveBaseline();
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
          if (!finishPipelineCreatePending(data.id, data.ok === true, data.error)) {
            if (!finishPipelineRenameAutosavePending(data.id, data.ok === true)) {
              if (!finishPipelineStepAutosavePending(data.id, data.ok === true)) {
                finishSkillAutosavePending(data.id, data.ok === true);
              }
            }
          }
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

      function primePipelineCreatePendingSet() {
        if (!window.__primePipelineCreatePending) {
          window.__primePipelineCreatePending = new Set();
        }
        return window.__primePipelineCreatePending;
      }

      function setPipelineCreateError(msg) {
        const el = document.querySelector("[data-testid='pipeline-create-error']");
        if (!el) {
          return;
        }
        if (msg == null || msg === "") {
          el.textContent = "";
          el.hidden = true;
        } else {
          el.textContent = msg;
          el.hidden = false;
        }
      }

      function finishPipelineCreatePending(msgId, ok, err) {
        const set = primePipelineCreatePendingSet();
        if (!set.has(msgId)) {
          return false;
        }
        set.delete(msgId);
        if (ok) {
          document.getElementById("pipeline-modal")?.close();
          setPipelineCreateError(null);
        } else {
          setPipelineCreateError(err != null ? String(err) : "Failed");
        }
        return true;
      }

      function getPipelineRenameFormState(form) {
        const key = form.getAttribute("data-old-name") || "";
        if (!pipelineRenameFormState.has(key)) {
          pipelineRenameFormState.set(key, {
            inFlight: false,
            lastSentPayload: "",
            renameDebounceId: null
          });
        }
        return pipelineRenameFormState.get(key);
      }

      function primePipelineRenamePendingMap() {
        if (!window.__primePipelineRenameAckPending) {
          window.__primePipelineRenameAckPending = new Map();
        }
        return window.__primePipelineRenameAckPending;
      }

      function buildPipelineRenamePayload(form) {
        const oldName = form.getAttribute("data-old-name") || "";
        const input = form.querySelector("input[name='name']");
        if (!input) {
          return null;
        }
        return oldName + "|" + input.value.trim();
      }

      function finishPipelineRenameAutosavePending(msgId, ok) {
        const pendingMap = primePipelineRenamePendingMap();
        const pending = pendingMap.get(msgId);
        if (!pending) {
          return false;
        }
        pendingMap.delete(msgId);
        if (pending.timeoutId) {
          window.clearTimeout(pending.timeoutId);
        }
        const st = pipelineRenameFormState.get(pending.formKey);
        if (st) {
          st.inFlight = false;
          if (ok) {
            st.lastSentPayload = pending.payload;
          }
        }
        return true;
      }

      function buildUpdateStepPayload(form) {
        const titleInput = form.querySelector("input[name='title']");
        const promptInput = form.querySelector("textarea[name='prompt']");
        if (!titleInput || !promptInput) {
          return null;
        }
        const payload = new URLSearchParams();
        payload.set("title", titleInput.value);
        payload.set("prompt", promptInput.value);
        return payload.toString();
      }

      function getPipelineStepFormState(form) {
        const pipeline = form.getAttribute("data-pipeline") || "";
        const stepId = form.getAttribute("data-step-id") || "";
        const key = pipeline + "|" + stepId;
        if (!pipelineStepFormState.has(key)) {
          pipelineStepFormState.set(key, {
            inFlight: false,
            lastSentPayload: ""
          });
        }
        return pipelineStepFormState.get(key);
      }

      function primePipelineStepPendingMap() {
        if (!window.__primePipelineStepAckPending) {
          window.__primePipelineStepAckPending = new Map();
        }
        return window.__primePipelineStepAckPending;
      }

      function finishPipelineStepAutosavePending(msgId, ok) {
        const pendingMap = primePipelineStepPendingMap();
        const pending = pendingMap.get(msgId);
        if (!pending) {
          return false;
        }
        pendingMap.delete(msgId);
        if (pending.timeoutId) {
          window.clearTimeout(pending.timeoutId);
        }
        const st = pipelineStepFormState.get(pending.formKey);
        if (st) {
          st.inFlight = false;
          if (ok) {
            st.lastSentPayload = pending.payload;
          }
        }
        return true;
      }

      function resyncPipelineStepAutosaveBaseline() {
        document.querySelectorAll("form[data-ws-form='update_step']").forEach((form) => {
          const p = buildUpdateStepPayload(form);
          if (p !== null) {
            getPipelineStepFormState(form).lastSentPayload = p;
          }
        });
      }

      function savePipelineStepForm(form) {
        const st = getPipelineStepFormState(form);
        if (st.inFlight) {
          return;
        }
        const titleInput = form.querySelector("input[name='title']");
        const promptInput = form.querySelector("textarea[name='prompt']");
        if (!titleInput || !promptInput) {
          return;
        }
        if (titleInput.value.trim() === "") {
          return;
        }
        if (promptInput.value.trim() === "") {
          return;
        }
        const payload = buildUpdateStepPayload(form);
        if (payload === null) {
          return;
        }
        if (payload === st.lastSentPayload) {
          return;
        }
        st.inFlight = true;
        try {
          const msgId = nextMsgId();
          const pipeline = form.getAttribute("data-pipeline") || "";
          const stepId = parseInt(form.getAttribute("data-step-id") || "0", 10);
          const afterPayload = buildUpdateStepPayload(form);
          if (afterPayload === null) {
            st.inFlight = false;
            return;
          }
          const pendingMap = primePipelineStepPendingMap();
          const timeoutId = window.setTimeout(() => {
            if (pendingMap.has(msgId)) {
              finishPipelineStepAutosavePending(msgId, false);
            }
          }, 2500);
          const formKey = pipeline + "|" + String(stepId);
          pendingMap.set(msgId, {
            formKey: formKey,
            payload: afterPayload,
            timeoutId: timeoutId
          });
          sendPrime({
            op: "update_step",
            id: msgId,
            pipeline: pipeline,
            step_id: stepId,
            title: titleInput.value,
            prompt: promptInput.value
          });
        } catch (_error) {
          st.inFlight = false;
        }
      }

      function savePipelineRenameForm(form) {
        const st = getPipelineRenameFormState(form);
        if (st.inFlight) {
          return;
        }
        const input = form.querySelector("input[name='name']");
        if (!input) {
          return;
        }
        const oldName = form.getAttribute("data-old-name") || "";
        const trimmed = input.value.trim();
        if (trimmed === "") {
          return;
        }
        if (trimmed === oldName.trim()) {
          const sync = buildPipelineRenamePayload(form);
          if (sync !== null) {
            st.lastSentPayload = sync;
          }
          return;
        }
        const payload = buildPipelineRenamePayload(form);
        if (payload === null) {
          return;
        }
        if (payload === st.lastSentPayload) {
          return;
        }
        st.inFlight = true;
        try {
          const msgId = nextMsgId();
          const afterPayload = buildPipelineRenamePayload(form);
          if (afterPayload === null) {
            st.inFlight = false;
            return;
          }
          const pendingMap = primePipelineRenamePendingMap();
          const timeoutId = window.setTimeout(() => {
            if (pendingMap.has(msgId)) {
              finishPipelineRenameAutosavePending(msgId, false);
            }
          }, 2500);
          pendingMap.set(msgId, {
            formKey: oldName,
            payload: afterPayload,
            timeoutId
          });
          sendPrime({
            op: "rename_pipeline",
            id: msgId,
            old_name: oldName,
            new_name: trimmed
          });
        } catch (_error) {
          st.inFlight = false;
        }
      }

      function schedulePipelineRenameAutosave(form) {
        const st = getPipelineRenameFormState(form);
        if (st.renameDebounceId != null) {
          window.clearTimeout(st.renameDebounceId);
        }
        st.renameDebounceId = window.setTimeout(() => {
          st.renameDebounceId = null;
          if (!form.isConnected) {
            return;
          }
          savePipelineRenameForm(form);
        }, 450);
      }

      function wirePipelineRenameDebouncedInputs() {
        document.querySelectorAll("form[data-ws-form='rename_pipeline']").forEach((form) => {
          if (form.dataset.primeRenameDebounced === "1") {
            return;
          }
          form.dataset.primeRenameDebounced = "1";
          const input = form.querySelector("input[name='name']");
          if (!input) {
            return;
          }
          input.addEventListener("input", () => {
            const st = getPipelineRenameFormState(form);
            if (st.renameDebounceId != null) {
              window.clearTimeout(st.renameDebounceId);
              st.renameDebounceId = null;
            }
            const nameInput = form.querySelector("input[name='name']");
            if (nameInput && nameInput.value.trim() === "") {
              const p = buildPipelineRenamePayload(form);
              if (p !== null) {
                st.lastSentPayload = p;
              }
              return;
            }
            schedulePipelineRenameAutosave(form);
          });
        });
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
        document.querySelectorAll("form[data-ws-form='rename_pipeline']").forEach((form) => {
          const initial = buildPipelineRenamePayload(form);
          if (initial !== null) {
            getPipelineRenameFormState(form).lastSentPayload = initial;
          }
        });
        document.querySelectorAll("form[data-ws-form='update_step']").forEach((form) => {
          const initial = buildUpdateStepPayload(form);
          if (initial !== null) {
            getPipelineStepFormState(form).lastSentPayload = initial;
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
            document.querySelectorAll("form[data-ws-form='rename_pipeline']").forEach((form) => {
              const p = buildPipelineRenamePayload(form);
              if (p === null) {
                return;
              }
              const st = getPipelineRenameFormState(form);
              if (st.inFlight) {
                return;
              }
              const nameInput = form.querySelector("input[name='name']");
              if (nameInput && nameInput.value.trim() === "") {
                st.lastSentPayload = p;
                return;
              }
              if (p === st.lastSentPayload) {
                return;
              }
              savePipelineRenameForm(form);
            });
            document.querySelectorAll("form[data-ws-form='update_step']").forEach((form) => {
              const p = buildUpdateStepPayload(form);
              if (p === null) {
                return;
              }
              const st = getPipelineStepFormState(form);
              if (st.inFlight) {
                return;
              }
              const titleInput = form.querySelector("input[name='title']");
              const promptInput = form.querySelector("textarea[name='prompt']");
              if (titleInput && titleInput.value.trim() === "") {
                st.lastSentPayload = p;
                return;
              }
              if (promptInput && promptInput.value.trim() === "") {
                st.lastSentPayload = p;
                return;
              }
              if (p === st.lastSentPayload) {
                return;
              }
              savePipelineStepForm(form);
            });
          }, 1000);
        }
        wirePipelineRenameDebouncedInputs();
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
            setPipelineCreateError(null);
            primePipelineCreatePendingSet().add(id);
            sendPrime({
              op: "create_pipeline",
              id: id,
              name: String(fd.get("name") || "")
            });
            return;
          }
          if (ws === "rename_pipeline") {
            savePipelineRenameForm(form);
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

      function wirePipelineCreateOpenClear() {
        if (window.__primePipelineCreateOpenClear) {
          return;
        }
        window.__primePipelineCreateOpenClear = true;
        document.addEventListener("click", (event) => {
          const t = event.target;
          if (!(t instanceof Element)) {
            return;
          }
          if (t.closest("[data-testid='pipeline-create-open']")) {
            setPipelineCreateError(null);
          }
        });
      }

      function wireAll() {
        wireLowercaseInputs();
        wireAutosave();
        wireDeleteConfirmations();
        wirePipelineStepReorder();
        wirePipelineCreateOpenClear();
        wireWsFormsOnce();
      }

      persistLastViewedFromLocation();
      maybeRedirectBareSectionUrl();
      connectPrimeWs();
      wireAll();
      wireTabLastViewed();
