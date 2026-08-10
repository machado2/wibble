// src/admin/Admin.tsx
import React from 'react';
import { Admin, ListGuesser, Resource, UpdateParams } from 'react-admin';
import { dataProvider } from "ra-data-simple-prisma";
import { authProvider } from './authProvider';
import { ContentList, ContentEdit, ContentCreate } from './Content';
import { ImageCacheList, ImageCacheEdit } from './ImageCache';

const prismaDataProvider = dataProvider('/api/adm', )

const dataProviderWithUpload = {
  ...prismaDataProvider,
  update: async (resource: string, params: UpdateParams) => {
    if (resource !== 'image_cache') {
      // fallback to the default implementation
      return prismaDataProvider.update(resource, params);
    }

    let newPicture = params.data.image_data;
    if (newPicture?.rawFile instanceof File) {
      const formData = new FormData();
      formData.append('file', newPicture.rawFile);
      const response = await fetch(`/api/adm/uploadimage/${params.id}`, {
        method: 'PUT',
        body: formData,
        credentials: 'same-origin',
      });
      if (!response.ok) {
        throw new Error(`Error uploading image: ${response.statusText}`);
      }
    }

    // Remove the image data from the params object before calling the default update handler
    delete params.data.image_data;
    return prismaDataProvider.update(resource, params);
  },
};

export const AdminPanel: React.FC = () => (
  <Admin dataProvider={dataProviderWithUpload}
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
