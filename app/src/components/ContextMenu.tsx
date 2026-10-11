/**
 * The menu a right click opens on its trigger.
 *
 * After LTK Manager's `src/components/ContextMenu.tsx`, over this app's `Menu` look. Cut: the
 * submenus, the radio and checkbox rows, the groups and the item variants.
 */
import { ContextMenu as BaseContextMenu } from "@base-ui/react/context-menu";
import { forwardRef, type ReactNode } from "react";

import { twMerge } from "@/utils";

import { menuIconClass, menuItemClass, menuPopupClass, menuSeparatorClass } from "./Menu";

export interface ContextMenuTriggerProps extends Omit<BaseContextMenu.Trigger.Props, "className"> {
  className?: string;
  children?: ReactNode;
}

const ContextMenuTrigger = forwardRef<HTMLDivElement, ContextMenuTriggerProps>(
  ({ className, children, ...props }, ref) => (
    <BaseContextMenu.Trigger ref={ref} className={className} {...props}>
      {children}
    </BaseContextMenu.Trigger>
  ),
);
ContextMenuTrigger.displayName = "ContextMenu.Trigger";

export interface ContextMenuContentProps extends Omit<BaseContextMenu.Popup.Props, "className"> {
  className?: string;
  children?: ReactNode;
}

/** Portal, Positioner and Popup as one part. */
const ContextMenuContent = forwardRef<HTMLDivElement, ContextMenuContentProps>(
  ({ className, children, ...props }, ref) => (
    <BaseContextMenu.Portal>
      <BaseContextMenu.Positioner className="z-50">
        <BaseContextMenu.Popup
          ref={ref}
          className={twMerge(menuPopupClass, className)}
          data-ui="ContextMenu"
          {...props}
        >
          {children}
        </BaseContextMenu.Popup>
      </BaseContextMenu.Positioner>
    </BaseContextMenu.Portal>
  ),
);
ContextMenuContent.displayName = "ContextMenu.Content";

export interface ContextMenuItemProps extends Omit<BaseContextMenu.Item.Props, "className"> {
  icon?: ReactNode;
  className?: string;
  children?: ReactNode;
}

const ContextMenuItem = forwardRef<HTMLDivElement, ContextMenuItemProps>(
  ({ icon, className, children, ...props }, ref) => (
    <BaseContextMenu.Item
      ref={ref}
      className={twMerge(menuItemClass, "text-surface-100", className)}
      {...props}
    >
      {icon && <span className={menuIconClass}>{icon}</span>}
      <span className="min-w-0 flex-1 truncate">{children}</span>
    </BaseContextMenu.Item>
  ),
);
ContextMenuItem.displayName = "ContextMenu.Item";

function ContextMenuSeparator() {
  return <BaseContextMenu.Separator className={menuSeparatorClass} />;
}

/** A right click's menu: `Root` around a `Trigger` and a `Content` of `Item`s and `Separator`s. */
export const ContextMenu = {
  Root: BaseContextMenu.Root,
  Trigger: ContextMenuTrigger,
  Content: ContextMenuContent,
  Item: ContextMenuItem,
  Separator: ContextMenuSeparator,
};
