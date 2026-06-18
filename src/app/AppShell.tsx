import { StoreIcon } from "lucide-react";
import { useState } from "react";

import { BackendStatus } from "@/app/BackendStatus";
import {
  foundationCards,
  navigationItems,
  type NavigationItemId,
} from "@/app/navigation";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarSeparator,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { PosServices } from "@/services/ports";

interface AppShellProps {
  services: PosServices;
}

export function AppShell({ services }: AppShellProps) {
  const [activeId, setActiveId] = useState<NavigationItemId>("register");
  const activeItem =
    navigationItems.find((item) => item.id === activeId) ?? navigationItems[0];

  return (
    <TooltipProvider>
      <SidebarProvider>
        <Sidebar collapsible="icon">
          <SidebarHeader>
            <div className="flex min-w-0 items-center gap-2 p-2">
              <div className="flex size-8 shrink-0 items-center justify-center rounded-md bg-sidebar-primary text-sidebar-primary-foreground">
                <StoreIcon aria-hidden="true" />
              </div>
              <div className="min-w-0 group-data-[collapsible=icon]:hidden">
                <div className="truncate text-sm font-medium">VantumPOS</div>
                <div className="truncate text-xs text-sidebar-foreground/70">
                  Lokalna kasa
                </div>
              </div>
            </div>
          </SidebarHeader>
          <SidebarContent>
            <SidebarGroup>
              <SidebarGroupLabel>Rad</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {navigationItems.map((item) => {
                    const Icon = item.icon;
                    const isActive = item.id === activeId;

                    return (
                      <SidebarMenuItem key={item.id}>
                        <SidebarMenuButton
                          type="button"
                          tooltip={item.label}
                          isActive={isActive}
                          aria-current={isActive ? "page" : undefined}
                          onClick={() => setActiveId(item.id)}
                        >
                          <Icon aria-hidden="true" />
                          <span>{item.label}</span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    );
                  })}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
          <SidebarSeparator />
          <SidebarFooter>
            <div className="flex flex-col gap-2 group-data-[collapsible=icon]:hidden">
              <Badge variant="outline">Smena nije otvorena</Badge>
              <div className="truncate px-2 text-xs text-sidebar-foreground/70">
                Admin
              </div>
            </div>
          </SidebarFooter>
        </Sidebar>
        <SidebarInset>
          <header className="flex min-h-14 shrink-0 items-center gap-3 px-4 py-3">
            <SidebarTrigger />
            <Separator orientation="vertical" className="h-6" />
            <div className="min-w-0 flex-1">
              <h1 className="truncate text-lg font-semibold">
                {activeItem.label}
              </h1>
              <p className="truncate text-xs text-muted-foreground">
                Jedna radnja, jedna kasa, lokalna baza
              </p>
            </div>
            <BackendStatus services={services} />
          </header>
          <Separator />
          <div className="flex flex-1 flex-col gap-4 p-4">
            <section className="grid gap-3 md:grid-cols-3">
              {foundationCards.map((card) => {
                const Icon = card.icon;

                return (
                  <Card key={card.title} size="sm">
                    <CardHeader>
                      <CardTitle>{card.title}</CardTitle>
                      <CardAction>
                        <Icon
                          aria-hidden="true"
                          className="text-muted-foreground"
                        />
                      </CardAction>
                    </CardHeader>
                    <CardContent>
                      <CardDescription>{card.description}</CardDescription>
                    </CardContent>
                  </Card>
                );
              })}
            </section>
            <Card className="flex-1">
              <CardHeader>
                <CardTitle>Radni modul</CardTitle>
                <CardDescription>
                  {activeItem.label} je pripremljen za rad u ovoj radnji.
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <div className="rounded-md bg-muted/40 p-4">
                  <p className="text-base font-medium">{activeItem.label}</p>
                  <p className="text-xs text-muted-foreground">
                    Podaci za ovaj deo kase bice dostupni kroz dnevni rad.
                  </p>
                </div>
                <div>
                  <Button variant="outline" type="button">
                    Spremno za modul
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  );
}
