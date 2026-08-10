// src/admin/Admin.tsx
import React from 'react';
import { Admin, ListGuesser, Resource } from 'react-admin';
import { dataProvider } from "ra-data-simple-prisma";
import { authProvider } from './authProvider';
import { ContentList, ContentEdit, ContentCreate } from './Content';
import { ImageCacheList, ImageCacheEdit } from './ImageCache';

const prismaDataProvider = dataProvider('/api/adm')

export const AdminPanel: React.FC = () => (
  <Admin dataProvider={prismaDataProvider}
    authProvider={authProvider}
  >
    <Resource
      name="content"
      list={ContentList}
      edit={ContentEdit}
      create={ContentCreate}
    />
    <Resource
      name="image_cache"
      list={ImageCacheList}
      edit={ImageCacheEdit}
    />
    <Resource name="search_history" list={ListGuesser} />
    <Resource name="history_generation_fail" list={ListGuesser} />
  </Admin>
);
