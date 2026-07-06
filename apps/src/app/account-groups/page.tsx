"use client";

import { FormEvent, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { PencilLine, Plus, RefreshCw, Save, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { useDesktopPageActive } from "@/hooks/useDesktopPageActive";
import {
  isAdminRole,
  resolveSessionRole,
  useAppSession,
} from "@/hooks/useAppSession";
import { usePageTransitionReady } from "@/hooks/usePageTransitionReady";
import { useRuntimeCapabilities } from "@/hooks/useRuntimeCapabilities";
import { accountClient } from "@/lib/api/account-client";
import { getAppErrorMessage } from "@/lib/api/transport";
import { useI18n } from "@/lib/i18n/provider";
import { useAppStore } from "@/lib/store/useAppStore";
import { formatTsFromSeconds } from "@/lib/utils/usage";
import type { AccountGroup } from "@/types";

const QUERY_KEY = ["account-groups"] as const;

function draftFromGroup(group: AccountGroup | null, nextSort: number) {
  return {
    name: group?.name ?? "",
    description: group?.description ?? "",
    status: group?.status || "active",
    sort: String(group?.sort ?? nextSort),
  };
}

function parseSort(value: string): number {
  const parsed = Number.parseInt(value.trim(), 10);
  return Number.isFinite(parsed) ? parsed : 0;
}

export default function AccountGroupsPage() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { isDesktopRuntime } = useRuntimeCapabilities();
  const { data: session, isLoading: isSessionLoading } = useAppSession();
  const role = resolveSessionRole(session, isSessionLoading, isDesktopRuntime);
  const isAdminMode = isAdminRole(role);
  const serviceConnected = useAppStore((state) => state.serviceStatus.connected);
  const isPageActive = useDesktopPageActive("/account-groups/");
  const shouldQuery = isAdminMode && serviceConnected && isPageActive;
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingGroup, setEditingGroup] = useState<AccountGroup | null>(null);
  const [draft, setDraft] = useState(draftFromGroup(null, 0));

  const groupsQuery = useQuery({
    queryKey: QUERY_KEY,
    queryFn: () => accountClient.listAccountGroups(),
    enabled: shouldQuery,
  });

  usePageTransitionReady(
    "/account-groups/",
    !shouldQuery || groupsQuery.isFetched || groupsQuery.isError,
  );

  const groups = useMemo(() => groupsQuery.data ?? [], [groupsQuery.data]);
  const nextSort = useMemo(
    () => groups.reduce((max, group) => Math.max(max, group.sort), 0) + 10,
    [groups],
  );

  const invalidateRelated = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: QUERY_KEY }),
      queryClient.invalidateQueries({ queryKey: ["accounts"] }),
      queryClient.invalidateQueries({ queryKey: ["apikeys"] }),
      queryClient.invalidateQueries({ queryKey: ["startup-snapshot"] }),
    ]);
  };

  const saveMutation = useMutation({
    mutationFn: () =>
      accountClient.saveAccountGroup({
        oldName: editingGroup?.name ?? null,
        name: draft.name,
        description: draft.description || null,
        status: draft.status,
        sort: parseSort(draft.sort),
      }),
    onSuccess: async () => {
      await invalidateRelated();
      setDialogOpen(false);
      toast.success(t("账号组已保存"));
    },
    onError: (error: unknown) => {
      toast.error(`${t("保存失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (name: string) => accountClient.deleteAccountGroup(name),
    onSuccess: async () => {
      await invalidateRelated();
      toast.success(t("账号组已删除"));
    },
    onError: (error: unknown) => {
      toast.error(`${t("删除失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  const openCreateDialog = () => {
    setEditingGroup(null);
    setDraft(draftFromGroup(null, nextSort));
    setDialogOpen(true);
  };

  const openEditDialog = (group: AccountGroup) => {
    setEditingGroup(group);
    setDraft(draftFromGroup(group, nextSort));
    setDialogOpen(true);
  };

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (!draft.name.trim()) {
      toast.error(t("账号组名称不能为空"));
      return;
    }
    saveMutation.mutate();
  };

  const handleDelete = (group: AccountGroup) => {
    const inUse = group.accountCount > 0 || group.apiKeyCount > 0;
    if (inUse) {
      toast.error(t("账号组已被账号或平台密钥使用，不能删除"));
      return;
    }
    if (window.confirm(t("确认删除该账号组？"))) {
      deleteMutation.mutate(group.name);
    }
  };

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-4 p-4 md:p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">{t("账号组管理")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {t("统一维护账号池和平台密钥可使用的账号组。")}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => groupsQuery.refetch()}
            disabled={!shouldQuery || groupsQuery.isFetching}
          >
            <RefreshCw className="mr-2 h-4 w-4" />
            {t("刷新")}
          </Button>
          <Button type="button" onClick={openCreateDialog} disabled={!serviceConnected}>
            <Plus className="mr-2 h-4 w-4" />
            {t("新建账号组")}
          </Button>
        </div>
      </div>

      <Card className="glass-card border-border/70">
        <CardHeader className="pb-3">
          <CardTitle className="text-base">{t("账号组")}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("组名")}</TableHead>
                  <TableHead>{t("描述")}</TableHead>
                  <TableHead>{t("状态")}</TableHead>
                  <TableHead className="text-right">{t("账号数量")}</TableHead>
                  <TableHead className="text-right">{t("密钥数量")}</TableHead>
                  <TableHead className="text-right">{t("排序")}</TableHead>
                  <TableHead>{t("更新时间")}</TableHead>
                  <TableHead className="w-[112px] text-right">{t("操作")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {groups.map((group) => (
                  <TableRow key={group.name}>
                    <TableCell className="font-medium">{group.name}</TableCell>
                    <TableCell className="max-w-[280px] truncate text-muted-foreground">
                      {group.description || t("未填写描述")}
                    </TableCell>
                    <TableCell>
                      <Badge variant={group.status === "active" ? "default" : "secondary"}>
                        {group.status === "active" ? t("启用") : t("停用")}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-right">{group.accountCount}</TableCell>
                    <TableCell className="text-right">{group.apiKeyCount}</TableCell>
                    <TableCell className="text-right">{group.sort}</TableCell>
                    <TableCell className="text-muted-foreground">
                      {group.updatedAt ? formatTsFromSeconds(group.updatedAt) : "-"}
                    </TableCell>
                    <TableCell>
                      <div className="flex justify-end gap-1">
                        <Button
                          type="button"
                          size="icon"
                          variant="ghost"
                          title={t("编辑")}
                          onClick={() => openEditDialog(group)}
                        >
                          <PencilLine className="h-4 w-4" />
                        </Button>
                        <Button
                          type="button"
                          size="icon"
                          variant="ghost"
                          title={t("删除")}
                          disabled={deleteMutation.isPending}
                          onClick={() => handleDelete(group)}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
                {groups.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={8} className="h-28 text-center text-muted-foreground">
                      {groupsQuery.isLoading ? t("正在加载...") : t("暂无账号组")}
                    </TableCell>
                  </TableRow>
                ) : null}
              </TableBody>
            </Table>
          </div>
        </CardContent>
      </Card>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{editingGroup ? t("编辑账号组") : t("新建账号组")}</DialogTitle>
          </DialogHeader>
          <form className="grid gap-4" onSubmit={handleSubmit}>
            <div className="grid gap-2">
              <Label htmlFor="accountGroupName">{t("组名")}</Label>
              <Input
                id="accountGroupName"
                value={draft.name}
                onChange={(event) =>
                  setDraft((current) => ({ ...current, name: event.target.value }))
                }
                autoFocus
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="accountGroupDescription">{t("描述")}</Label>
              <Textarea
                id="accountGroupDescription"
                value={draft.description}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    description: event.target.value,
                  }))
                }
                rows={3}
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-2">
                <Label>{t("状态")}</Label>
                <Select
                  value={draft.status}
                  onValueChange={(value) =>
                    setDraft((current) => ({
                      ...current,
                      status: String(value || "active"),
                    }))
                  }
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value="active">{t("启用")}</SelectItem>
                      <SelectItem value="disabled">{t("停用")}</SelectItem>
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </div>
              <div className="grid gap-2">
                <Label htmlFor="accountGroupSort">{t("排序")}</Label>
                <Input
                  id="accountGroupSort"
                  inputMode="numeric"
                  value={draft.sort}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, sort: event.target.value }))
                  }
                />
              </div>
            </div>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => setDialogOpen(false)}
              >
                {t("取消")}
              </Button>
              <Button type="submit" disabled={saveMutation.isPending}>
                <Save className="mr-2 h-4 w-4" />
                {t("保存")}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
