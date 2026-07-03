"use client";

import { useEffect, useRef, useState } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Copy,
  FileCog,
  KeyRound,
  Link2,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useI18n } from "@/lib/i18n/provider";
import { cn } from "@/lib/utils";
import { copyTextToClipboard } from "@/lib/utils/clipboard";

interface CodexCliOnboardingDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAcknowledge: (dismissPermanently: boolean) => Promise<void>;
}

const GUIDE_STEPS = [
  {
    icon: FileCog,
    title: "第一步：确认服务已连接",
    description: "先确认 CodexManager 本地服务可用，再写 Codex 配置。",
    details: [
      "顶部或设置页显示“服务已连接”。",
      "默认网关地址是 `http://localhost:48760/v1`。",
    ],
  },
  {
    icon: KeyRound,
    title: "第二步：填写 auth.json",
    description: "把平台密钥写入 Codex 的 `auth.json`，不要填账号 token。",
    details: [
      "先到“平台密钥”页面复制一个可用 Key。",
      "Windows 默认路径：`%USERPROFILE%\\.codex\\auth.json`。",
    ],
  },
  {
    icon: Link2,
    title: "第三步：填写 config.toml 后重启 Codex",
    description: "复制基础配置即可；如果改过服务端口，只改 `base_url`。",
    details: [
      "Windows 默认路径：`%USERPROFILE%\\.codex\\config.toml`。",
      "保存后关闭并重新打开 Codex。",
    ],
  },
] as const;
const GUIDE_AUTH_JSON_TEXT = `{
  "OPENAI_API_KEY": "replace_with_codexmanager_platform_key",
  "auth_mode": "apikey"
}`;

const GUIDE_CONFIG_LINES = [
  {
    comment: "让 Codex 使用下面这个本地 provider",
    line: 'model_provider = "codex"',
  },
  {
    comment: null,
    line: "",
  },
  {
    comment: "provider 名称必须和上面的 model_provider 一致",
    line: "[model_providers.codex]",
  },
  {
    comment: "显示名称",
    line: 'name = "Codex"',
  },
  {
    comment: "CodexManager 本地网关地址",
    line: 'base_url = "http://localhost:48760/v1"',
  },
  {
    comment: "使用 Responses 协议",
    line: 'wire_api = "responses"',
  },
] as const;
export function CodexCliOnboardingDialog({
  open,
  onOpenChange,
  onAcknowledge,
}: CodexCliOnboardingDialogProps) {
  const { t } = useI18n();
  const [currentStep, setCurrentStep] = useState(0);
  const [dismissPermanently, setDismissPermanently] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const introFocusRef = useRef<HTMLDivElement | null>(null);
  const scrollContainerRef = useRef<HTMLDivElement | null>(null);
  const codeBlockRef = useRef<HTMLPreElement | null>(null);
  const activeStep = GUIDE_STEPS[currentStep];
  const isFirstStep = currentStep === 0;
  const isLastStep = currentStep === GUIDE_STEPS.length - 1;
  const guideAuthJson = GUIDE_AUTH_JSON_TEXT;
  const guideConfig = GUIDE_CONFIG_LINES.map(({ comment, line }) => {
    if (!line) {
      return "";
    }
    if (!comment) {
      return line;
    }
    return `# ${t(comment)}\n${line}`;
  }).join("\n");
  const guideClipboardText = [
    "# ~/.codex/auth.json",
    guideAuthJson,
    "",
    "# ~/.codex/config.toml",
    guideConfig,
  ].join("\n");

  useEffect(() => {
    if (!open) {
      return;
    }

    setCurrentStep(0);
    const resetScroll = () => {
      scrollContainerRef.current?.scrollTo({
        top: 0,
        left: 0,
        behavior: "auto",
      });
      codeBlockRef.current?.scrollTo({ top: 0, left: 0, behavior: "auto" });
    };

    resetScroll();
    const rafId = window.requestAnimationFrame(resetScroll);
    return () => window.cancelAnimationFrame(rafId);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    scrollContainerRef.current?.scrollTo({ top: 0, left: 0, behavior: "auto" });
  }, [currentStep, open]);

  const handleAcknowledge = async () => {
    setIsSaving(true);
    try {
      await onAcknowledge(dismissPermanently);
      setDismissPermanently(false);
    } finally {
      setIsSaving(false);
    }
  };

  const handleSessionClose = () => {
    setDismissPermanently(false);
    onOpenChange(false);
  };

  const handleRequestClose = () => {
    if (dismissPermanently) {
      void handleAcknowledge();
      return;
    }
    handleSessionClose();
  };

  const handleOpenChange = (nextOpen: boolean) => {
    if (isSaving) {
      return;
    }
    if (!nextOpen) {
      handleRequestClose();
      return;
    }
    onOpenChange(nextOpen);
  };

  const handleCopyConfig = async () => {
    try {
      await copyTextToClipboard(guideClipboardText);
      toast.success(t("配置模板已复制"));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleCopySnippet = async (text: string) => {
    try {
      await copyTextToClipboard(text);
      toast.success(t("配置片段已复制"));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent
        initialFocus={introFocusRef}
        className="glass-card mission-panel max-h-[min(82dvh,760px)] overflow-hidden p-0 sm:!max-w-[min(92vw,980px)]"
      >
        <div className="flex max-h-[min(82dvh,760px)] min-h-0 flex-col">
          <DialogHeader className="shrink-0 border-b border-border/60 px-5 py-4">
            <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
              <div
                ref={introFocusRef}
                tabIndex={-1}
                className="max-w-2xl select-none space-y-1 outline-none"
              >
                <DialogTitle className="text-lg font-semibold md:text-xl">
                  {t("Codex 首次接入引导")}
                </DialogTitle>
                <DialogDescription className="text-xs leading-5 md:text-sm">
                  {t(
                    "只需要准备 `auth.json` 和 `config.toml` 两个文件。没有勾选“不再显示”时，下次进入软件仍会看到它。",
                  )}
                </DialogDescription>
              </div>
              <div className="rounded-md border border-border/60 bg-background/50 px-3 py-2 text-xs leading-5 text-muted-foreground lg:max-w-xs">
                {t(
                  "右侧只保留基础配置；复制后按实际端口改 `base_url` 即可。",
                )}
              </div>
            </div>
          </DialogHeader>

          <div
            ref={scrollContainerRef}
            className="grid min-h-0 flex-1 gap-3 overflow-y-auto px-5 py-4 xl:grid-cols-[minmax(0,1fr)_minmax(340px,0.86fr)]"
          >
            <div className="min-h-0 space-y-3">
              <section className="rounded-md border border-border/60 bg-background/45 p-4 shadow-sm">
                <div className="flex flex-col gap-3 border-b border-border/50 pb-3">
                  <div className="space-y-1">
                    <h3 className="text-base font-semibold leading-7 text-foreground">
                      {t("基础步骤")}
                    </h3>
                    <p className="text-sm leading-6 text-muted-foreground">
                      {t("你当前在第 {current} 步，共 {total} 步。", {
                        current: currentStep + 1,
                        total: GUIDE_STEPS.length,
                      })}
                    </p>
                    <p className="text-xs leading-5 text-muted-foreground">
                      {t("按顺序完成这三项即可。")}
                    </p>
                  </div>
                  <div className="grid gap-2 md:grid-cols-3">
                    {GUIDE_STEPS.map((step, index) => (
                      <Button
                        key={step.title}
                        type="button"
                        variant="outline"
                        onClick={() => setCurrentStep(index)}
                        className={cn(
                          "h-auto min-w-0 justify-start rounded-md px-3 py-2.5 text-left transition-colors",
                          index === currentStep
                            ? "border-primary/30 bg-primary/10 text-foreground shadow-sm"
                            : "border-border/60 bg-background/70 text-muted-foreground hover:bg-accent/50",
                        )}
                      >
                        <div className="text-xs font-semibold">
                          {t("步骤 {step}", { step: index + 1 })}
                        </div>
                        <div className="mt-1 line-clamp-2 text-sm font-medium leading-6">
                          {t(step.title)}
                        </div>
                      </Button>
                    ))}
                  </div>
                </div>

                <div className="mt-3">
                  <section className="rounded-md border border-border/60 bg-background/70 p-4">
                    <div className="flex items-start gap-4">
                      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-md border border-primary/15 bg-primary/10 text-primary">
                        <activeStep.icon className="h-5 w-5" />
                      </div>
                      <div className="min-w-0 space-y-2">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="rounded-md bg-primary/10 px-2.5 py-1 text-xs font-semibold text-primary">
                            {t("步骤 {step}", { step: currentStep + 1 })}
                          </span>
                          <h4 className="text-base font-semibold leading-7 text-foreground">
                            {t(activeStep.title)}
                          </h4>
                        </div>
                        <p className="text-sm leading-6 text-muted-foreground">
                          {t(activeStep.description)}
                        </p>
                        <ul className="list-disc space-y-1 pl-5 text-sm leading-6 text-muted-foreground">
                          {activeStep.details.map((detail) => (
                            <li key={detail}>{t(detail)}</li>
                          ))}
                        </ul>
                      </div>
                    </div>
                  </section>
                </div>
              </section>
            </div>

            <section className="min-h-0 rounded-md border border-border/60 bg-background/55 shadow-sm">
              <div className="flex flex-col gap-3 border-b border-border/60 px-4 py-3 sm:flex-row sm:items-start sm:justify-between">
                <div className="space-y-1">
                  <h3 className="text-base font-semibold leading-7 text-foreground">
                    {t("基础配置示例")}
                  </h3>
                  <p className="text-sm leading-6 text-muted-foreground">
                    {t(
                      "只包含 Codex 接入 CodexManager 所需的最小配置。",
                    )}
                  </p>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="gap-2 self-start rounded-md"
                  onClick={() => void handleCopyConfig()}
                >
                  <Copy className="h-4 w-4" />
                  {t("复制配置")}
                </Button>
              </div>
              <div className="space-y-3 p-4">
                <div className="space-y-2">
                  <div className="flex flex-wrap items-center justify-between gap-3 text-sm font-semibold text-foreground">
                    <div className="space-y-1">
                      <div>{t("auth.json 示例")}</div>
                      <div className="text-xs font-normal text-muted-foreground">
                        {t("这个 Key 来自 CodexManager 的“平台密钥”页面")}
                      </div>
                    </div>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="h-8 gap-2 px-2 text-xs"
                      onClick={() => void handleCopySnippet(guideAuthJson)}
                    >
                      <Copy className="h-3.5 w-3.5" />
                      {t("复制 auth.json")}
                    </Button>
                  </div>
                  <pre className="max-h-[14dvh] overflow-auto rounded-md border border-slate-900/80 bg-slate-950 p-3 font-mono text-xs leading-6 text-slate-100">
                    <code>{guideAuthJson}</code>
                  </pre>
                </div>
                <div className="space-y-2">
                  <div className="flex items-center justify-between gap-3 text-sm font-semibold text-foreground">
                    <span>{t("config.toml 示例")}</span>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="h-8 gap-2 px-2 text-xs"
                      onClick={() => void handleCopySnippet(guideConfig)}
                    >
                      <Copy className="h-3.5 w-3.5" />
                      {t("复制 config.toml")}
                    </Button>
                  </div>
                  <pre
                    ref={codeBlockRef}
                    className="max-h-[20dvh] overflow-auto rounded-md border border-slate-900/80 bg-slate-950 p-3 font-mono text-xs leading-6 text-slate-100"
                  >
                    <code>{guideConfig}</code>
                  </pre>
                </div>
              </div>
            </section>
          </div>

          <DialogFooter className="mx-0 mb-0 mt-auto shrink-0 rounded-b-lg border-t border-border/60 bg-background/95 px-5 py-3 sm:flex-nowrap sm:items-center sm:justify-between">
            <label className="flex items-center gap-3 pr-4 text-sm text-muted-foreground">
              <Checkbox
                checked={dismissPermanently}
                onCheckedChange={(checked) =>
                  setDismissPermanently(Boolean(checked))
                }
                disabled={isSaving}
                aria-label={t("下次不再显示这份引导")}
              />
              <span className="leading-6">{t("下次不再显示这份引导")}</span>
            </label>
            <div className="flex shrink-0 flex-col-reverse gap-2 sm:flex-row">
              {!isLastStep ? (
                <>
                  {!isFirstStep ? (
                    <Button
                      type="button"
                      variant="outline"
                      className="gap-2"
                      onClick={() =>
                        setCurrentStep((step) => Math.max(0, step - 1))
                      }
                      disabled={isSaving}
                    >
                      <ChevronLeft className="h-4 w-4" />
                      {t("上一步")}
                    </Button>
                  ) : null}
                  <Button
                    type="button"
                    className="gap-2"
                    onClick={() =>
                      setCurrentStep((step) =>
                        Math.min(GUIDE_STEPS.length - 1, step + 1),
                      )
                    }
                    disabled={isSaving}
                  >
                    {t("下一步")}
                    <ChevronRight className="h-4 w-4" />
                  </Button>
                </>
              ) : null}
              {!dismissPermanently ? (
                <Button
                  type="button"
                  variant="outline"
                  onClick={handleSessionClose}
                  disabled={isSaving}
                >
                  {t("本次关闭")}
                </Button>
              ) : null}
              {isLastStep || dismissPermanently ? (
                <Button
                  type="button"
                  onClick={() => void handleAcknowledge()}
                  disabled={isSaving}
                >
                  {isSaving
                    ? t("保存中...")
                    : dismissPermanently
                      ? t("保存并关闭")
                      : t("我已阅读")}
                </Button>
              ) : null}
            </div>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}
